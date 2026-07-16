# W2-254: Require an authoritative non-empty range for every Task PR operation

## Problem

The ancestry proof from W2-138/W2-255 (`verify_task_pr_range`) proves parity
`M == B` but never proves the range is **non-empty**. The only emptiness guard
lives in `request_task_pr_publication` as `task_pr_has_changes`, and it has two
holes:

1. **First-publication only.** `if pr.github().is_none() && !task_pr_has_changes`
   — once a PR has a GitHub number, an update that resets or rebases the branch
   empty sails through to `gh pr edit`/`ready`/`merge`.
2. **Non-authoritative.** `task_pr_has_changes` compares against
   `origin/<default>...HEAD` (symmetric three-dot), which recomputes the
   merge-base on the fly. The authoritative range is `pr.base_commit..HEAD`
   (the durable recorded base). A drift between the symmetric merge-base and
   the recorded base lets an empty range pass (or a real one refuse).

Stacked children widen the hole: `verify_task_pr_range_with_authority` always
resolves upstream as `origin/<default>`, so a child stacked on an unmerged
parent measures its range against main — the parent's commits look foreign and
the child's own work is never checked against its durable fork point.

## Design

One shared verifier with two checks, both authoritative (store + session +
recorded `base_commit`), called at the point where the range is finally
determined (after commit + rebase, before any `gh` call):

- **Ancestry parity** (existing `verify_task_pr_range_with_authority`): `M == B`
  after healing. Runs before the first push to refuse contaminated ancestry.
- **Non-empty range** (new): after parity holds, `git diff --quiet {base}..HEAD`
  against the healed recorded base must report a tree change. Empty → refuse
  before any `gh pr create/edit/ready/merge`.

### Why two call points

- **Pre-push** (before `commit_workflow`/`prepare_land`): ancestry only. Work
  may be uncommitted; an emptiness check here would refuse real uncommitted
  work. The ancestry check must precede the push so a contaminated branch never
  reaches the remote.
- **Pre-`gh`** (after commit + rebase): ancestry + non-empty. The range is now
  final; refusing here precedes every `gh` mutation (`create`, `edit`, `ready`,
  `merge --auto`).

### Stacked children

`task_stack` already resolves the durable fork/parent boundary
(`StackedRebase.fork_base` = `pr.base_commit`, `parent_branch`). The verifier
uses it to pick the upstream:

- **Child with live parent**: upstream = parent's branch tip. Parity is
  `merge-base(parent_tip, HEAD) == base_commit`. Non-empty is
  `base_commit..HEAD` — parent-only changes cannot satisfy the child.
- **Child after parent merged** (parent_branch is None): upstream =
  `origin/<default>`. The collapse already rebased the child onto main and
  healed the base; the verifier confirms parity + non-empty against main.

This reuses W2-93's stack semantics (`task_stack`, `record_stack_rebase`) — no
duplicate stack logic.

### Mutation paths covered

| path                          | pre-push ancestry | pre-gh non-empty |
|-------------------------------|:-:|:-:|
| `lf pr publish`/`open` (create_or_update_pr) | ✓ | ✓ |
| `lf pr land` (prepare_pr → ensure_pr → create_or_update_pr) | ✓ | ✓ |
| `lf pr land` updating an existing PR (prepare_pr → finalize_remote) | ✓ | ✓ |
| `lf pr submit` (same as land, AssignForReview) | ✓ | ✓ |

`request_task_pr_publication` loses its `task_pr_has_changes` guard (replaced by
the shared verifier) and the `pr.github().is_none()` condition (emptiness is
unconditional now).

## The demo

Infrastructure-only — no user-facing demo. The proof is the test matrix: an
empty range is refused before any `gh` call on every mutation path, and a
stacked child measures from its durable fork point, not main.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does the implementation compile and pass existing tests? | `cargo check` clean; 39 unit + 7 integration tests pass with the uncommitted changes. | Implementation is sound; only test coverage is missing. |
| Are all four mutation paths actually wired? | `pr.rs:130` (publish/open) and `land.rs:76` (land/submit) both call `require_task_pr_range_nonempty` after rebase, before any `gh` call. `prepare_pr` covers create + update + finalize_remote since the check precedes `ensure_pr` and `finalize_remote`. | Confirmed — no bypass path. |
| Is `task_pr_has_changes` fully removed? | Deleted from `task.rs`; its only caller (`request_task_pr_publication`) lost the guard. No remaining references. | Clean removal, no dead code. |
| Does `resolve_verifier_upstream` correctly handle a collapsed child? | When `parent.merge_commit.is_some()` or `abandoned_at.is_some()`, it falls through to `resolve_upstream_base` (origin/default) — the collapse already rebased the child onto main. | Matches the design; covered by the stacked-child test. |
| Does the empty check use the healed base? | `require_task_pr_range_nonempty_with_authority` calls `verify_task_pr_range_with_authority` first (which heals), then re-reads the PR from the store before diffing. | Authoritative — the diff is against the post-heal recorded base, not a recomputed merge-base. |
| Can `git diff --quiet` exit non-0/non-1 on error? | The `status.code()` is only checked in the old `task_pr_has_changes`; the new code uses `status.success()` (exit 0 = empty = refuse). A git error surfaces as `OpsError` from `status()`. | Simpler and correct: 0 = no diff = empty. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep emptiness in `request_task_pr_publication`, fix the two holes | One guard, two patches. Still non-authoritative unless rewritten to use `base_commit`. | Rewriting it to be authoritative = the shared verifier. Doing it in two places duplicates the base-read + heal logic. |
| Single call point (pre-gh only, drop pre-push ancestry) | Fewer calls. But a contaminated branch reaches the remote push before the `gh` refusal — the push itself is a side effect. | The pre-push ancestry gate prevents contaminated ancestry from ever reaching origin. Two call points are load-bearing. |
| Measure emptiness with `git log` (commit count) instead of `git diff --quiet` | Catches "no commits" but misses "commits that produce no tree change" (revert-to-base). | `diff --quiet` catches both: an empty range and a range whose net tree change is zero. |

## Key decisions

- **Two call points, not one.** Pre-push = ancestry only (work may be
  uncommitted). Pre-`gh` = ancestry + non-empty (range is final). Collapsing
  them would either refuse real uncommitted work or let contamination reach the
  remote.
- **`git diff --quiet {base}..HEAD` for emptiness, not commit count.** A branch
  rebased back to its base has zero tree change even if it carries commits;
  `diff --quiet` catches that, `log` does not.
- **Re-read the PR after healing.** The ancestry check may heal `base_commit`
  forward. The non-empty check re-reads from the store so it diffs against the
  healed base, not a stale in-memory copy.
- **Stacked upstream via `resolve_verifier_upstream`, not `task_stack`.** The
  verifier needs the parent's *tip* (for merge-base), not just the fork base.
  `task_stack` resolves the fork boundary; the verifier resolves the live
  parent tip from the store's `parent_pr_id` → `get_task_pr`.

## Scope

- In scope: shared authoritative verifier (ancestry + non-empty), wired into all
  four mutation paths; `task_pr_has_changes` removed; stacked-child upstream
  resolution; integration + unit tests for the full matrix.
- Out of scope: changing the pre-push ancestry gate's behavior (W2-138/W2-255
  already shipped it); non-Task PRs (the verifier is a no-op without a Task
  Session).

## Implementation status

Implementation is in the working tree (uncommitted) and compiles clean. Existing
tests pass. Remaining: the Done-when test matrix needs two scenarios that no
test exercises yet:

- **Empty range refusal** — an existing PR (with a GitHub number) reset/rebased
  empty must refuse before any `gh` call. This is the core hole this task closes.
- **Stacked child** — live parent (child measures from parent tip, parent-only
  changes can't satisfy) and collapsed parent (child measures from origin/main
  after collapse). The `stacked_rotation_task` helper exists but no test calls
  it.

## Done when

- All four mutation paths share one authoritative verifier that refuses an
  absent or empty range before any remote mutation.
- Integration tests cover: new PR, existing PR re-published empty, serial
  rotation, stacked child (live parent + collapsed), empty range, stale base,
  squash-merged parent.
