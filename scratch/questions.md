# W2-156 — status, blockers & control-plane defects

## RESOLVED (2026-07-15): the control-plane store recovered

The shared `~/.lf/loopflow.db` was rebuilt onto `0.11.004_task_pr_ci_state`
(product's, on main) — the divergent `context_launch_work` row is gone and `lf`
store commands work again. Directive v2 was acknowledged; PR2 build resumed.

## PR2 progress (branch ci-fix-wake)

- **Slice 1 — committed `529b19888`:** the `(head, failure-set)` dedup key on
  `CiObservation` — `woken_failure_set` + `wake_warranted()` + `mark_woken()` +
  reconcile carry-forward. Pure, tested
  (`ci_wake_dedup_fires_once_per_head_and_failure_set`). Locks the wake's hardest
  correctness property: exactly one wake per `(head, failure_set)`.
- **Slice 2 — next:** the wake execution. `LaunchIntent::CiFix` past the open-PR
  `supervisor_restart_bar`, gated on `wake_warranted()`; the supervisor (project
  `inspect_outcome`, which already reconciles its tasks) launches a
  Waiting-on-open-PR task with that intent when the fresh reading warrants it; the
  runner loads a single-step `ci-fix.yaml` (seeded from the observation's PR#,
  branch, head_sha, failing checks + log URLs) instead of `task.yaml`, marks the
  observation woken, then rearms to Waiting on the new head.
- **Slice 3 — next:** the `reconcile_process_liveness` queue bridge (W2-144 gen 7),
  the `lf status` "fixing CI" owner refinement (failing + live ci-fix generation →
  Task), and the deterministic integration harness proving the full contract.

PR2 is **not landable** until slices 2–3 land — the wake is not demoable on slice 1
alone. Do not `lf pr land` this branch yet.

## Control-plane defect recorded (governance fix still wanted)

Two waves independently minted ordinal `0.11.004`: product's
`0.11.004_task_pr_ci_state` (merged, PR #916) and Context Lab's
`0.11.004_context_launch_work` (`06025c3eb`, branch `jack-heart/context-lab`, not
on main). A Context-Lab-built worker migrated the shared `~/.lf/loopflow.db` to its
`0.11.004`, bricking the release/product `lf` until the store was rebuilt.
`validate_set` forces Context Lab to renumber to `0.11.005` on rebase onto main
(two `(0,11,4)` ids are rejected), so main self-heals — but the durable fix is
**per-wave migration ordinal ranges** (or `LF_HOME=~/.lf-dev` dev-store isolation)
so two waves can never pick the same ordinal. Recorded here + in wave memory once
reachable; not a W2-156 code change.

## Pre-existing (carried from PR1)

`pr_tests.rs::github_failure_leaves_publication_intent_observable` fails on this
sandbox at the pre-change baseline too — a git-in-sandbox environment issue, not a
regression.
</content>
