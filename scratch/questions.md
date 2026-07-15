# W2-156 — status, blockers & control-plane defects

## RECURRING BLOCKER (2026-07-15): shared control-plane store re-bricked by a Context Lab in-flight migration

The store recovered once (rebuilt onto `0.11.004_task_pr_ci_state`), then broke
**again**: it now carries `0.11.006_context_launch_work`, unknown to the product
`lf` (latest known `0.11.005_provider_accounts`). Context Lab renumbered its
migration (0.11.004 → 0.11.006) but its worker **still applies that unmerged
migration to the shared `~/.lf/loopflow.db`**, so every non-Context-Lab `lf`
(release, product) rejects the store and `lf task acknowledge` / `lf pm` / land
fail.

**Renumbering is not the fix — it just moves the collision.** The root cause is a
wave running its in-flight schema against the *shared* ledger. The durable fix is
**dev-store isolation**: point in-development builds at `LF_HOME=~/.lf-dev` (or
per-wave stores) so a wave's unmerged migrations never touch the real control
plane. Until that lands, any wave dogfooding a schema change bricks `lf` for every
other wave on the machine. Not a W2-156 code change; recorded for the owner of the
control-plane store isolation (the `lf store: isolate dev and release control-plane
stores` line of work, #908).

Impact on W2-156: `cargo` tests (temp DBs) are unaffected, so slice 2 code is
buildable, but the PR cannot be acknowledged/landed via `lf` while the store is
bricked. Do not repair the shared store from here (recovery context).

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
