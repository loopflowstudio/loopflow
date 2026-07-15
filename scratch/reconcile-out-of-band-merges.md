# Reconcile out-of-band merges & un-ignored scratch-stash (W2-171)

Two avoidable-repair regressions surfaced landing W2-116 (PR #905). Both block
`lf task complete` on a fully-shipped Task. Both are narrow, testable fixes.

## User-visible outcome

A developer (or headless runner) whose PR merged **out of band** — GitHub
auto-merge armed by `lf pr land` (not settled by `lf pr land -c`) — can run
`lf task complete <issue> --summary "..."` and it succeeds. No manual repair,
no `lf pr abandon` on a merged branch, no hand-deleting a stash directory.

## Root causes

1. **Wrong PR chosen during reconcile.** `current_or_merged_pr_for_branch`
   (`ops/pr.rs`) runs `gh pr list --head <branch> --state all` and takes
   `list.into_iter().next()` — the newest PR. A land can leave a stray
   open/draft sibling on the branch (W2-116 saw empty draft #909 beside merged
   #905). `.next()` returns the draft, so `reconcile_task_pr` sees state
   `open`/`draft`, never sets `merge_commit`, and the TaskPr row stays
   unsettled. `active_task_pr` keeps returning it, and `task_complete`
   (`ops/task.rs:1746`) refuses with "Task has an open pull request".

2. **Scratch stash lands outside the ignored prefix.** `scratch_stash_path`
   (`ops/rebase.rs:272`) builds `.lf/scratch-stash/<branch>-<ts>/`. The ignored
   siblings are `.lf/{log,logs,prompts,journal,summaries,tmp}` — `scratch-stash`
   is **not** ignored. `lf pr land` restores scratch there, dirtying the
   worktree, and `task_complete` (`ops/task.rs:1734`) refuses with "Task
   worktree has uncommitted changes". Wave memory already documents the path as
   `.lf/tmp/scratch-stash/` — the code drifted out from under the ignored `tmp/`.

## Source of truth

- Merge state: GitHub, read via `gh pr list`. The authoritative *persisted*
  record is the `task_prs` row (`TaskPr`); reconcile projects GitHub's merge
  onto it by setting `merge_commit` and appending a `PrMerged` event. Task
  events are **append-only** — reconcile adds, never rewrites (existing
  `should_append` guards preserve this).
- Stash location: derived, host-local scratch under the already-ignored
  `.lf/tmp/` prefix. Not a wire record.

## The fixes

### Fix 1 — prefer the merged PR (ops/pr.rs)

Replace the arbitrary `.next()` with a pure selection that ranks the branch's
PRs: **merged > open/draft > closed**, newest as tiebreak within a rank. A
merged PR is the branch's truth; a stray draft/closed sibling is noise.

Extract a pure `fn select_reconcile_pr(prs: Vec<GhPr>) -> Option<GhPr>` so it is
unit-testable without `gh`. `current_or_merged_pr_for_branch` parses the JSON,
calls it, and maps the winner to `PrInfo` exactly as today.

Only the reconcile read path (`current_or_merged_pr_for_branch`) changes.
`find_open_pr` / `current_pr` (open-review flows) are untouched — narrow scope,
PR-state ownership stays with W2-169.

### Fix 2 — stash under the ignored prefix (ops/rebase.rs)

`scratch_stash_path`: `.join("scratch-stash")` → `.join("tmp").join("scratch-stash")`.
One line. Matches the documented, already-gitignored path. No `.gitignore`
edit needed (chosen over adding `.lf/scratch-stash/` to `.gitignore` because it
converges code with the documented path and keeps one stash root under `tmp/`).

## End-to-end proof

**Scenario (Fix 1):** a TaskPr row is `Open` with a merged PR **and** an
open-draft sibling on the same branch. Reconcile must set `merge_commit` from
the merged PR and settle the row; `active_task_pr` then returns `None` and
`task_complete` succeeds. Proven by a unit test on `select_reconcile_pr`
(merged wins over a newer draft) plus a `reconcile_task_pr` test that drives a
two-PR branch through to a settled, completable session (mock `gh` output;
assert row `merge_commit` set, `PrMerged` event appended once, no prior events
rewritten).

**Scenario (Fix 2):** after a reset that stashes `scratch/`, `git status` is
clean — the stash sits under `.lf/tmp/`. Proven by a unit test asserting
`scratch_stash_path` is under `.lf/tmp/` and that a stashed-and-restored tree
leaves no untracked path outside the ignored prefix.

## Affected surfaces / consumers

- `lf task complete` — now proceeds on an out-of-band merged PR. Behavior only
  broadens (a previously-refused, fully-shipped Task completes).
- `lf pr status` / task runner reconcile calls — share
  `reconcile_task_pr`; they gain correct merge observation for free.
- `lf pr land` / `lf rebase` — stash path moves under `.lf/tmp/`.
- No wire DTO change, no migration: `TaskPr` fields and the `task_prs` schema
  are unchanged; only which GitHub PR feeds reconcile, and where scratch lands.

## Absent / error states

- Branch has **no** PR → `current_or_merged_pr_for_branch` returns `None`,
  reconcile no-ops (unchanged).
- Branch has only an open PR → selection returns it, session stays `Waiting`
  (unchanged review flow).
- gh reports merged **without** a merge commit → existing hard error in
  `reconcile_task_pr` (`ops/task.rs:1358`) stands; we do not silently settle.
- `gh` unavailable → `current_or_merged_pr_for_branch` returns `None` today;
  unchanged (reconcile simply can't observe — not this Task's problem).

## Operational boundary

Reconcile already shells one `gh pr list`; selection is in-memory over that one
result. No new subprocess, no extra network round-trip.

## Exclusions

- Broader PR-state ownership / lifecycle redesign → **W2-169**.
- No `--force` on `lf task complete`; no change to the abandon path.
- No cleanup of stray sibling PRs (e.g. auto-closing empty draft #909-likes) —
  reconcile just ignores them by preferring the merged PR.
- The `task_complete` "open pull request" check (`ops/task.rs:1746`) is left
  as-is: once Fix 1 settles the row, `active_task_pr` filters it out, so the
  check is never reached for a merged PR.
