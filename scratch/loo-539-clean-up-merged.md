# LOO-539: prune finished branches from origin

Status: PR #1616 merged 2026-10-21 (merge 46c84f46). This worktree carries an
uncommitted post-merge compress follow-up: one `protection()` shared by the
classifier and the executor's filter, the summary line as `BranchPruneReport`'s
`Display`, `deleted(copy)` replacing `count(copy, status)`, `clock` made private,
`#[non_exhaustive]` dropped from two crate-private enums, and an unneeded
`branch != ''` filter removed from `read_task_branches` (both columns are
`NOT NULL`; an empty name matches no ref). It needs its own PR in this Task.
Remaining: publish that follow-up, the real (non-dry) run with second-run
idempotence for demo, and Jack's call on retired release debris
(`questions.md`). Product direction is Jack's, from the LOO-539 brief.

## Problem

Branches from finished work pile up on GitHub and in local refs. Jack's hourly
cleanup script (LOO-538) needs one lf command it can call next to the machine
cleanup. The command deletes only branches that lf can tie to finished work. It
never deletes a branch with an open PR or a live worktree. It supports a dry run,
logs every deletion, and fails visibly when GitHub authentication or the rate
limit stops it.

## Demo

```
$ lf wt prune --branches --dry-run
would delete origin jack-heart/discord  closed PR #1315 at d71f8c8
would delete origin release-candidate/default/v0-14-1/a00544e…  released v0.14.1 at a00544e
would delete local  jack/loo-480-…  merged PR #1540 at 3be01aa
…
retained origin jack-heart/re-evaluate-usage-attribution-after  head moved past merged PR #1394 at baa53e6
retained origin jack-heart/agent-86d674b8  open PR #1588 at 3febf84
retained origin jack-heart/design-subwaves-and-expressive-memory  named by an unfinished Task at e15e3a0
retained origin jack-heart/publish-default-v0-13-0  no finished-work evidence at …
branches: would delete 11 remote, 120 local; retained 148; failed 0; GitHub GraphQL 4979/5000 left, resets 14:56
```

Running it without `--dry-run` prints the same lines with `deleted` in place of
`would delete`. Afterwards `git ls-remote --heads origin | wc -l` drops from 53 to
about 42, and a second run deletes nothing. Run with a revoked token
(`GH_TOKEN=bad lf wt prune --branches`), it deletes nothing, writes
`GitHub authentication failed: …; nothing deleted` to stderr and exits 1.

A dev build hands off to the installed lf unless `LF_HOME` names a private home.
Demo it from a build with `LF_HOME=$(mktemp -d)`; Task protections still come
from the production store, read without migrations.

## Risk findings (probed 2026-10-20 against loopflowstudio/loopflow)

- **Auto-delete on merge already happens.** The repository has
  `delete_branch_on_merge: true`. Of 53 remote heads, only one is a merged-PR
  branch (`re-evaluate-usage-attribution-after`), and its head `baa53e6` moved
  past PR #1394's head `35e2bb6`. So the head-equality rule retains that branch.
  Deleting it would lose those commits.
- **Most remote debris comes from releases.** 10 `release-candidate/<target>/<tag>/<commit>`
  heads come from the current release code. `delete_candidate_ref` only warns when its
  delete fails (`ops/release.rs:4898`). Each of those heads equals the `<commit>` in its
  name, and each tag exists. Another 36 `jack-heart/{prepare,publish}-default-v0-1x-*`
  branches come from release code that has since been retired. Their heads are already
  in `main`, but no current lf record names them. See Exclusions and `questions.md`.
- **Local refs are the larger pile.** The main checkout has 223 local branches and 30
  worktrees. Of the branches without a worktree, 114 have a MERGED or CLOSED PR at
  exactly their head, and 27 are contained in `origin/main`. 20 moved past a finished
  PR and 31 have no evidence. All remote-tracking refs are current
  (`git remote prune --dry-run origin` prints nothing), but the command still prunes
  them after it deletes branches.
- **GitHub failures are silent today.** `remote_stdout`
  (`git/worktrees.rs:455`) sends stderr to `/dev/null` and turns every failure into
  `RemoteFailure::Unavailable`. A missing login, a locked keychain under launchd and
  an exhausted rate limit all look the same as a network hiccup. `gh` exits with code 4
  when authentication is required. Rate limiting shows up as HTTP 403/429 with "rate
  limit" in stderr, or as a GraphQL error of `type: RATE_LIMITED`. `gh api rate_limit`
  is free and reports the GraphQL budget, so each run can log it.
- **Rate-limit budget.** About 250 branch names, asked 16 per aliased GraphQL request
  (`GITHUB_BRANCHES_PER_REQUEST`), take about 16 requests. Each request costs about 1
  point. With two machines running hourly, that is under 1k points a day against
  5k/hour. Budget only becomes a concern when something else has drained it, and then
  the run must say so.
- **Laptop and mini run concurrently.** `ops::wt::apply_delete` already pushes
  `--force-with-lease=refs/heads/B:H`. If the other machine deleted the branch first,
  or someone pushed to it, the lease rejects the push. The command then re-reads the
  remote head: if the branch is gone it reports `already gone`, and if the head moved
  it reports `retained: head moved`. Neither case is an error.
- **Other machines' worktrees.** A worktree on the mini cannot be seen from the
  laptop. That branch is still protected when this machine's store has its Task in a
  non-terminal state, or when GitHub has an open PR for it. A branch that has neither
  but has a closed PR at its head is deleted. The mini keeps its local branch and can
  push it again.

## Approach

Add `lf wt prune --branches [--dry-run]`, the third mode of the existing prune command
alongside `--builds`. `wt` already owns branch deletion (`wt delete` removes the local
and remote branch). The command runs against the current repository, and the hourly
script calls it once per repository.

### Evidence (a branch is deleted only with one of these)

Here B is the branch and H is its head: the remote head for the `origin` copy, and the
local head for the local ref. Each copy is judged separately.

| Reason | Condition |
|---|---|
| `merged-pr` / `closed-pr` | GitHub has no OPEN PR for B, and has a MERGED/CLOSED PR whose `headRefOid == H` |
| `finished-task` | A Task naming B (`tasks.branch` or any `task_prs.branch`) is Done or Abandoned, and H is an ancestor of `origin/<default>` |
| `released` | B parses as a `ReleaseCandidate` branch, H equals its `<commit>`, and its tag exists on origin |

### Protections (checked first; they override any evidence)

These branches are never deleted:

- the default branch
- a branch checked out in any worktree on this machine
- a branch with any OPEN PR, at any head
- a branch named by a non-terminal Task in the registry, or in the production store
  read without migrations (`task_ownership`)

A branch with no evidence is retained with the reason `no finished-work evidence`. A
branch whose head moved past its evidence is retained with the reason `head moved past …`.

### Failure behavior (fail closed, loudly)

- If GitHub PR facts are unavailable for any reason, nothing is deleted. The command
  prints a classified one-line reason (`authentication failed: …`,
  `rate limited: <detail>`, `timed out after Ns`, or `unavailable: <detail>`) and
  exits 1. Without those facts it cannot rule out an open PR.
- If the GraphQL budget is below 200 remaining before the run, nothing is deleted. The
  command prints `GitHub GraphQL budget N left until HH:MM; skipped` and exits 1.
- A registry it cannot read is handled the way `wt prune` already handles it:
  an error, and nothing deleted. A missing registry means no Task evidence and no Task
  protections.
- A deletion that fails is printed as `failed <ref>: <git stderr>`. The command keeps
  going with the other branches and exits 1 at the end.
- Stdout has one line per action and a summary line. That covers Jack's "log what was
  deleted", because the script captures stdout and stderr. The command also writes
  `record_ops_metric({"op":"wt.prune.branches", deleted_remote, deleted_local, retained,
  failed, graphql_remaining})`.

### Mechanics

1. **Budget first.** One free `gh api rate_limit` call runs before any query. It
   surfaces auth failures before any GraphQL work and supplies the budget for the
   skip check and the summary line.
2. **Shared GitHub branch facts.** In `git/worktrees.rs`, `github_request` and its
   parser return the raw per-branch PR nodes (`number, headRefOid, state`). PR
   precedence at a head (open > merged > closed) lives once in
   `worktrees::head_pull_request`; `wt list` derives its `PullRequestState` from it
   and prune reads "any OPEN" and "finished at H" from the same nodes. One query, one
   parser.
3. **Classified remote failures.** `remote_stdout` captures stderr. `RemoteFailure` is
   `Auth`, `RateLimited { detail }`, `TimedOut` or `Unavailable { detail }`. `wt list`
   maps all of them to its existing `RemoteOutcome` without changing the wire format.
4. **Task protections and evidence.** `task_ownership` (`lf/commands/ops/mod.rs`) is
   the one Task read for both `wt prune` and `wt prune --branches`. It returns
   protected paths and `TaskBranches`, whose `FromIterator` folds live over finished.
   Production-store branches come from `store::read_task_branches`, which shares
   `task_state_sql` with `read_task_worktree_ownership`.
5. **Classifier.** A pure function in `ops/branch_prune.rs` takes the facts (remote
   heads, local branches, worktree branches, PR nodes, Task branches, release tags via
   `release::released_candidate_tag`, and an `is_ancestor` closure) and returns one
   `BranchPruneEntry` per copy. Its `Display` is the log line.
6. **Deletion.** Each copy is its own lease delete through
   `ops::wt::branch_copies_at`/`apply_delete`, remote first, then local, so a lost
   remote lease cannot misreport the local copy. `apply_delete` leaves a `None` copy
   alone. The local land path uses the same lease delete at the merged head.
   `git remote prune origin` runs last.
7. **CLI.** In `WtCommand::Prune`, `--branches` conflicts with `--builds`; build-only
   flags keep `requires = "builds"`.

## Source of truth and consumers

- Truth: GitHub PR state, read live at each run; the Task registry's Work status and
  branches; origin's heads and tags. The command adds no new records.
- Consumers: Jack's LOO-538 script runs the command and reads its exit code and
  stdout. `wt list` stays wire-compatible and only shares the fetch code. The Desktop
  app and DTOs are unaffected; `--json` is excluded.

## Exclusions

- **Retired release debris:** 36 `jack-heart/{prepare,publish}-default-*` branches.
  Teaching the classifier names from deleted release code would keep that dead scheme
  alive. Dry runs list them as `no finished-work evidence`. Getting rid of them is a
  one-time decision for Jack (see `questions.md`).
- Repositories other than the current one. The script loops over them.
- `--json` output, and any scheduling or launchd installation, which belong to LOO-538.
- Changing the worktree-pruning rules in `wt prune`.

## Deleted (done)

- `git::delete_remote_branch`, which had no callers.
- `ops/land.rs::delete_remote_branch`; `finalize_local` now lease-deletes at the
  merged head, so there is one way to delete a remote branch.
- `parse_pull_request_states`' per-branch collapse, replaced by `head_pull_request`
  over the shared PR nodes.
- Bounded and kept: `release::delete_candidate_ref` keeps its own lease-equivalent
  delete, because it runs under `ReleaseLock`'s command wrapper. Prune now cleans up
  whatever that delete leaks.

## Forbidden outcomes

- Deleting on missing or partial GitHub facts, or on name patterns alone.
- A second GraphQL client, a second PR parser, or a second Task-protection reader.
- Exit 0 when authentication, the rate limit or a deletion failed.
- Deleting with a plain `push --delete` that has no lease on the observed head.

## Slices

All three slices shipped in #1616; the follow-up above is a compress pass only.

## Done when

- A dry run against loopflowstudio/loopflow lists the closed-PR branch and the
  released candidate branches for deletion. It retains every open-PR branch, every
  live-Task branch and `re-evaluate-usage-attribution-after`, each with its reason.
- A real run deletes exactly the branches the dry run listed, and a second run
  deletes nothing.
- When `gh` is unauthenticated or rate limited, the run exits 1, prints the classified
  reason and leaves every ref untouched.
- Acceptance, run headless at gate: `cargo test -p loopflow --test integration wt_prune_branches -- < /dev/null`
  with `LF_*` unset. The stub-`gh` fixture covers three cases:
  (a) a closed PR at the head and a release candidate whose tag exists → both deleted on
  origin and locally, and a branch with an open PR and a branch with moved head both
  retained;
  (b) a stub that exits 4 → exit 1, stderr contains `authentication failed`, origin
  heads unchanged;
  (c) a stub that returns `RATE_LIMITED` → exit 1, `rate limited`, unchanged.
  Expected: all pass. A fourth case covers the low-budget skip.

Checks: realign 2026-10-20: real-repo dry run (`LF_HOME=$(mktemp -d) target/debug/lf wt prune --branches --dry-run`) exit 0, 11 remote / 120 local, retentions as listed in Done when; compress pass: clippy clean, `--lib branch_prune git::worktrees` 29 pass, `wt_prune_branches` 4/4 pass; gate owns the full matrix. The real (non-dry) run and second-run idempotence remain for demo.

Gate 2026-10-20: auth failures now read `authentication failed: <gh stderr>` (a 401 "Bad credentials" exits 1, not 4, so the old `(gh exit 4)` label was wrong for the revoked-token demo); `cargo fmt --check` + `cargo clippy --all-targets -D warnings` pass; `--lib git::worktrees ops::branch_prune ops::wt ops::land ops::release store::sqlite` 247 pass; `--test integration wt_prune_branches land wt_` 47 pass. Full matrix deferred to CI.
