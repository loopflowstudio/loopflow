# W2-172 — Task control through PR observation failures

Keep a Task worker's durable work and local control intact when GitHub
observation is stale, rate-limited, or an earlier PR merged out of band.
Resuming or rotating never silently drops committed or uncommitted follow-up.

Builds on PR #913 (out-of-band merge reconciliation via `select_reconcile_pr`,
`lf pr next` dirty-carry, scratch-stash under `.lf/tmp/`). Does **not** duplicate
those. One durable Task/PR state model — no second cache, no compat shim.

## Two dogfood failures this closes

1. **Committed follow-up dropped on rotate (W2-166).** After PR #907 merged,
   the worker committed *unique* follow-up commits on the still-checked-out
   sequence-1 branch. `lf pr next` (PR #913) carries only the *dirty* working
   tree; it rotates the new branch from `origin/main` and leaves those committed
   commits behind. Recovering them today means manual `git` surgery.
2. **Local control dies on a remote read (2026-07-15).** Exhausted GitHub
   GraphQL quota killed W2-93/W2-151/W2-170 at flow boundaries. Root cause:
   `reconcile_task_pr_with_authority` unconditionally calls
   `current_or_merged_pr_for_branch` → `gh pr list --head <branch> --state all`
   (a GraphQL enumeration), and it runs at the top of *every* control command
   (`follow_up`, `steer`, `interrupt`, `status`, `resume`, `complete`,
   `pr next`). A `?` on that read hard-fails the whole command. Durable Task
   control became dependent on a live remote read.

## Source of truth

The persisted `TaskPr` row (`store.active_task_pr` / `task_prs`) is the single
authority for a Task's PR state — branch, base commit, `publication.github`
number, `merge_commit`, sequence. GitHub is a *reconciliation input*, never the
store of record. All CLI/DTO/app views derive from the `TaskPr` row; a remote
read only updates it, and a failed read leaves it untouched.

## PR1 — Bounded, REST-first, degradation-tolerant observation

**User-visible outcome.** When GitHub is quota-exhausted, offline, or erroring,
`lf task status|follow-up|steer|interrupt|resume` still succeed against cached
`TaskPr` state and each surfaces observation freshness plus the exact degraded
reason. When a persisted PR number exists, reconcile issues exactly one bounded
REST read; it never enumerates PRs. An unpublished working PR (no persisted
number) triggers no remote read at all.

**Design.**

- **REST-by-number, no enumeration.** Replace the reconcile-time call to
  `current_or_merged_pr_for_branch` (GraphQL `gh pr list`) with a bounded read
  keyed on `pr.publication.github.number`:
  `gh api repos/{owner}/{repo}/pulls/{number}` (REST, single object). Parse
  `state`, `merged`, `merge_commit_sha`, `head.sha`, `html_url`, `draft`. Owner
  and repo come from the existing repo→nwo resolver (`github_repo_nwo` in
  `engine/worktrees.rs`). A working PR with no `publication.github` skips the
  read and returns the cached row unchanged — this is the "do not enumerate
  remote PRs for unpublished working PRs" clause, satisfied structurally.
  - `gh pr list` (enumeration) stays only where a branch has *no* recorded
    number and we genuinely must discover a PR — i.e. `current_pr` /
    `pr_exists_for_current_branch` for the operator-facing `lf pr` surface, not
    the Task reconcile hot path. Reconcile only ever has a number-or-nothing.
- **Observation freshness, not a hard error.** Introduce a small result type
  returned by the remote read:

  ```
  enum PrObservation {
      Fresh(PrInfo),                 // remote confirmed
      NotFound,                      // number 404s (deleted) — treat as absent
      Degraded { reason: String },   // quota/network/gh failure; cache stands
  }
  ```

  Classify `gh` exit + stderr into `Degraded` for the credential/quota/network
  family (rate-limit "API rate limit already exceeded", HTTP 403/429/5xx, exit
  from a killed/again transport) — reuse the bounded-ssh classification spirit
  already in the codebase. `Degraded` never propagates as `OpsError`.
- **Reconcile preserves cache on Degraded.** `reconcile_task_pr_with_authority`
  returns the cached `TaskPr` unchanged on `Degraded`, and records the freshness
  on the session so callers can surface it. Add to `TaskSession` (or its derived
  status view — decide during pursue, prefer the derived view so it isn't
  persisted stale) an `observation: Observation` field:
  `Fresh { at }` | `Degraded { reason, cached_as_of }`. This is a *derived view*
  computed per reconcile, not a new durable cache — honors "one state model."
- **Control commands stop gating on the read.** `queue_command`, `task_status`,
  `task_resume` already call `reconcile_task_pr` first; with Degraded folded in,
  the reconcile is now infallible for transport failures and the commands
  proceed on cache. The degraded reason rides back on `TaskControlResult` /
  `TaskSession` status view.

**Absent & error states.**
- No `gh` binary → `Degraded { reason: "gh CLI not found" }`, cache stands.
- Number 404 → `NotFound`; the PR ref was deleted remotely. Do not fabricate a
  merge; leave the cached settled/working state and note it. (A merged PR whose
  branch was deleted still has `merge_commit` persisted from the prior fresh
  read, so completion is unaffected.)
- Quota/network/5xx → `Degraded`; every local control still works.

**Operational boundary.** One REST call per reconcile, `ConnectTimeout`-bounded
like the existing ssh probes (~10s ceiling); never blocks a control command past
that; zero calls when there's no persisted number.

**End-to-end proof.**
- Integration test: seed a Task with a published `TaskPr` (number set); stub the
  `gh` transport to emit the rate-limit signature; assert `task_follow_up` /
  `task_steer` / `task_status` all succeed, the queued command lands, and the
  returned status carries `Degraded { reason }` naming the quota. (GraphQL
  exhaustion case.)
- Integration test: stub REST returning HTTP 5xx → same resilience. (REST
  failure case.)
- Integration test: working `TaskPr` with no number → assert zero `gh`
  invocations. (Unpublished working PR case.)
- Integration test: REST returns `merged` + `merge_commit_sha` → cached row gains
  `merge_commit`, phase becomes Merged. (Fresh out-of-band merge.)

## PR2 — Carry committed follow-up on rotate; close the completion/directive race

**User-visible outcome.** `lf pr next` after an out-of-band merge rotates to the
next serial branch carrying **both** the full unique post-merge commit range and
any dirty edits — no committed work is lost, no manual `git` surgery. And a
replacement directive accepted before an armed auto-merge fires is never silently
erased by completion.

**Design — carry the committed range.**

- **Merged branch tip is the boundary.** The REST read (PR1) yields `head.sha` —
  the branch tip GitHub merged. Persist it on the `TaskPr` at merge observation
  (`merged_head: Option<String>`, set alongside `merge_commit`). The follow-up
  range is `merged_head..<settled-branch HEAD>`: commits the worker added on the
  merged branch *after* the merge, which are exactly what rotation must carry.
  Ancestry alone can't separate them (a squash merge leaves the branch's own
  commits as non-ancestors of `main`, and per-commit patch-ids don't match the
  squash, so `git cherry`/`squash_merge_fork_point` misclassify a multi-commit
  branch) — the remote-authoritative `head.sha` is the reliable cut.
- **Rotation replays the range.** In `ensure_working_pr_with_authority`, after
  creating the next branch from `origin/{default}` (unchanged), if
  `merged_head` is set and `merged_head..settled_HEAD` is non-empty, cherry-pick
  that range onto the new branch (`git cherry-pick merged_head..settled_HEAD`),
  preserving the dirty working tree (checkout `-b` already carries it). A
  cherry-pick conflict aborts cleanly and returns a named error telling the
  operator to resolve on the settled branch — never a half-applied rotation.
- `carry_dirty` (PR #913) is unchanged; the committed-range carry is additive and
  runs on the same `lf pr next` path. The runner path (`carry_dirty = false`)
  still refuses to rotate a dirty tree, but now also carries the committed range
  when it *does* rotate a clean tree with follow-up commits.

**Design — completion never erases an accepted directive.**

- `TaskSession` already tracks `current_directive_version` and
  `incorporated_directive_version`. Define *pending directive* as
  `current_directive_version > incorporated_directive_version`.
- **Guard both completion paths.** In `task_complete` and in the auto-merge
  branch of `reconcile_task_pr_with_authority` (`AfterMerge::CompleteTask`):
  if a directive is pending, do **not** transition to `Completed`. Instead hold
  at `Waiting` with reason "directive vN accepted after land; incorporate it or
  re-steer before completing", keep the PR observed-as-merged, and leave the
  pending directive visible on the follow-up path. The merge is still recorded
  (truth), only the *completion* is withheld until the directive is incorporated
  (`lf task acknowledge`) or explicitly superseded.

**Absent & error states.**
- `merged_head` unset (older row, or merge never observed fresh) →
  fall back to today's behavior (carry dirty only); log that committed-range
  carry was skipped for lack of a recorded tip rather than silently dropping.
- Empty `merged_head..HEAD` → no cherry-pick, plain rotation.
- Cherry-pick conflict → abort, named error, worktree left on settled branch.
- No pending directive at completion → completes exactly as today.

**End-to-end proof.**
- Integration test (W2-166 shape): build a repo where a sequence-1 branch is
  squash-merged, then commits F1/F2 land on it *plus* an uncommitted edit;
  reconcile observes the merge (records `merged_head`); `lf pr next` produces a
  sequence-2 branch from `main` containing F1/F2 *and* the dirty edit, with no
  duplicate of the merged work. Recovers the W2-166 committed range.
- Integration test (dirty-only continuation): no committed follow-up, only dirty
  edits → sequence-2 carries the dirty edit (PR #913 behavior preserved).
- Integration test (completion/directive race): steer bumps
  `current_directive_version` past incorporated, then the armed auto-merge
  reconcile fires → assert the Task holds at `Waiting` with the pending-directive
  reason, not `Completed`; after `acknowledge`, completion proceeds.

## Affected surfaces & consumers

- `rust/loopflow/src/ops/pr.rs` — REST-by-number read + `PrObservation`
  classification; keep `select_reconcile_pr`/enumeration only on the operator
  `lf pr` discovery path.
- `rust/loopflow/src/ops/task.rs` — `reconcile_task_pr_with_authority`
  (Degraded-tolerant, records freshness, sets `merged_head`);
  `ensure_working_pr_with_authority` (committed-range cherry-pick);
  `task_complete` + auto-merge branch (pending-directive guard).
- `rust/loopflow/src/task/mod.rs` — `TaskPr.merged_head`; derived
  `Observation` on the status view (not a persisted second cache).
- `rust/loopflow/src/engine/git.rs` — a `cherry_pick_range` helper if none fits.
- Wire DTO: if observation freshness surfaces to Swift (`lf task status --json`
  / `waves.rs` snapshots), add the field to the DTO + fixture per the no-defaults
  DTO rule (required-or-explicit-Optional), and update each language's fixture
  test. Decide during pursue whether freshness crosses the wire this Task or
  stays CLI-text only; prefer CLI-text first, DTO only if a consumer needs it.
- Store migration: `TaskPr.merged_head` is a new nullable column — one additive
  migration, no rewrite of existing rows.

## Exclusions

- No change to how `lf pr land`/`submit` arm auto-merge (PR #913 owns that).
- No retry/backoff or quota-budgeting of GitHub reads — degrade, don't retry
  (matches the wave-memory guidance: don't tight-poll a shared GraphQL budget).
- No second product surface (Cadenza) in this Task.
- Recovering W2-166's *actual* worktree is the acceptance demo (operational,
  once), not a code deliverable here (see `scratch/questions.md`).
- Broader PR-state ownership beyond these paths stays with W2-169.

## Pursue target

PR1 first (bounded REST-first + degradation), with the four resilience tests
green, opened and left waiting for CI. Then PR2 (committed-range carry + race
guard) with the three tests green, including the W2-166-shaped recovery proof.
