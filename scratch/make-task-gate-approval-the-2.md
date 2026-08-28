# LOO-162 — Make the Task gate the sole human shipping decision

## LOO-247 recovery observations (2026-08-28)

- LOO-247 was durably stacked on LOO-162 PR sequence 2 before LOO-162 was
  completed. LOO-162's PR #1233 then merged as `2a242ddd9`; its worktree was
  correctly retired afterward. Later mainline integration retained that merge
  in history but overwrote the managed-submit implementation.
- `lf pr submit` in the LOO-247 worktree refused before publication, but at the
  ancestry proof: its recorded fork `1d9928750` is the pre-squash parent head
  and is not an ancestor of main. This is safe, but it does not prove the new
  managed-submit refusal.
- `lf rebase --plan` could not collapse the now-merged stack because
  `task_stack` called `reconcile_task_pr` for the parent before reading the
  already-recorded `merge_commit`; reconciliation tried to lock the retired
  parent's deleted worktree.
- After stack recovery, `lf rebase` reset LOO-247 to `origin/main` at
  `696082a9b` and restored this scratch note. That mainline contains the #1233
  merge in its ancestry, but no longer contains `reject_managed_task_submit`.
  Its authority test instead expects managed Task submission to succeed, and
  the user and architecture docs again teach a human merge click for Tasks.
- On the merged #1233 implementation, the refusal ran after `prepare_land`.
  A dirty managed worktree could therefore be committed or rebased locally
  before the remote-safe refusal. The current restoration moves the authority
  check ahead of every local, durable, and remote mutation.

## Recovery cause (verified 2026-08-28)

The earlier stale-integration hypothesis was false. Architecture source commit
`c1f78f397`, merged as #1236 (`fa0186c4d`), is a descendant of #1233 and
deliberately removed the refusal. Its design classified the Task `finally`
policy as controller authorization over lower-layer delivery, its review named
removal as a positive result, and its tests, docs, prompt goldens, and demo were
all changed to require managed submit. The causal analysis and follow-up
preventions are recorded in `scratch/refuse-human-submit-inside-managed.md`.

## Restoration verification (2026-08-28)

Observed passes:

- `cargo test -p loopflow --test task_pr_authority_tests
  managed_task_submit_refuses_before_any_mutation -- --exact` — 1 passed;
  stale `base_commit`, HEAD, dirty worktree state, durable merge request, push,
  and GitHub mutation all stay unchanged.
- `cargo test -p loopflow --test task_pr_authority_tests
  ordinary_submit_still_assigns_for_review -- --exact` — 1 passed.
- `cargo test -p loopflow --test golden_prompt` — 1 passed.
- `cargo fmt --all -- --check` — clean.
- `cargo clippy -p loopflow --all-targets -- -D warnings` — clean.
- `uv run --project website python scripts/render_architecture_html.py --check`
  — generated architecture reference is current.
- A current-source `target/debug/lf pr submit`, using a consistent backup of the
  live registry in an isolated temporary Home, resolved this real worktree as
  managed Task LOO-247 and refused with the `lf pr land -c` / `--next` action.
  It exited 1 with HEAD and worktree status unchanged. The live registry was
  read only; neither live store was migrated or replaced.

The initial behavior-test run was blocked before its surface by unsettled
machine install switch `switch-07a61ce74b514c2cb9ad61047ec9cc5d`. Durable Ask
`ask_8479b1a9757542f9973c5101c8fc88f1` recovered it without discarding either
store, after which every focused proof passed.

## LOO-247 recovery action

Recreate the retired parent path through `lf wt`, use the existing stacked
rebase to collapse the child onto main and clear the parent link, then remove
the temporary worktree. A permanent change that treats an already-recorded
parent `merge_commit` as sufficient without touching its worktree belongs to
the follow-up 5 Whys, not this restoration pass.

## Where this lands

The Run-centered architecture and the finally/ship landing model this Task
originally proposed remain on main. Concretely, current main already:

- Models the Task lifecycle as `First → Loop → Finally` (`task/mod.rs`), with the
  pinned **finally flow (`ship`) owning landing and Task completion**. The gate
  proposal is a small `TaskGateProposal { done, reason }`.
- SHA-pins auto-merge and **refuses to arm an unpinned merge**
  (`ops/land.rs::finalize_remote`, `ops/pr::enable_auto_merge(..head_sha)`).
- Records a durable, replay-safe `PrMergeRequest { mode, head_sha, after_merge,
  next_slug }` on the PR publication, is idempotent for an already-armed exact
  head (`matching_task_pr_auto_merge_request`), and rolls back on finalize failure
  (`clear_task_pr_merge_before_head_mutation`).
- Gates direct completion on an approved final gate (#1183) and completes a Task
  over an already-merged PR without an empty GitHub PR (#1135,
  `find_discardable_final_task` / `task_complete_approved`).
- Expresses shipping authority through Run/Review evidence, not a foundational
  Session lease.

So the earlier 1,443-line, Session-coupled branch is obsolete. The restored gap
that still lets a **second human shipping decision** exist is:

## The one real gap: `submit` on a managed Task

`lf pr submit` runs `prepare_pr(.., Finalize::UserMerge, ..)`, which for a
managed Task still writes a durable `PrMergeRequest { mode: User }`, marks the
PR ready, assigns it to a human, and prints *"Ready to land — click merge on the
PR once checks pass."* (`lf/commands/ops/mod.rs::submit_current`).

That merge click is exactly the redundant second gate LOO-162 removes: a managed
Task already carries one shipping judgment — its finally/`ship` gate, reviewed in
the provider-backed conversation and declared with `lf pr land -c` / `--next`.

## Change

1. **Refuse `submit` inside a managed Task worktree**, before any mutation.
   `ops/task::reject_managed_task_submit(repo)` resolves Task authority; if this
   is a managed Task, it errors with an actionable message pointing at
   `lf pr land -c` / `lf pr land --next <slug>`. `prepare_pr` calls it at the top
   when `finalize == Finalize::UserMerge`. Ordinary non-Task PRs are untouched
   (the exclusion "keep submit for non-Task PRs" holds).

   Separation-of-duties (an explicitly configured policy that *does* want a human
   merge click) is out of scope here — there is no such config today; noted in
   questions.md. When one exists it gates this refusal.

2. **Prompts + docs teach one sequence.** LOOPFLOW.md, the Task skills, and the
   `submit`/`land` op skills should teach: publish evidence during work, review
   in the provider session, declare with `lf pr land`, let GitHub execute and
   settle — and must not teach a managed Task to `submit` or click Merge.

Nothing else in the land path needs to change — SHA-pinning, replay safety,
complete/next disposition, review-gated completion, and finally-owned landing
already exist.

## Absent / error states (all already covered on main except #1)

- managed `submit` → **new** refusal, before mutation, names `lf pr land`.
- no PR / stale range / empty range → existing `verify_task_pr_range` /
  `require_task_pr_range_nonempty`.
- changed head after approval → existing exact-head pin in `request_task_pr_auto_merge`
  + `--match-head-commit`.
- failed CI → existing repair lifecycle; land refuses to complete without merge.
- restart between approval and land → existing idempotent
  `matching_task_pr_auto_merge_request` + durable `PrMergeRequest`.
- missing registry authority → existing `registry_authority_error`.

## Done when

- `cargo test -p loopflow --test task_pr_authority_tests` proves managed
  `submit` is refused before local, durable, or remote mutation, and ordinary
  non-Task `submit` still assigns for review.
- Prompt goldens / builtin assertions updated so LOOPFLOW.md and Task skills
  teach the single land sequence with no managed merge click.
- `cargo fmt`, `cargo clippy -p loopflow --all-targets -D warnings`, golden
  prompt test pass.
