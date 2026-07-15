# Open questions / notes — W2-156

## Pre-existing test failure (not introduced by this Task)

`rust/loopflow/tests/pr_tests.rs::github_failure_leaves_publication_intent_observable`
fails on this sandbox **at the pre-change baseline commit** too (verified by
checking out the clarify commit's source and running it). It asserts a
publication intent survives a failed `gh pr create`; the failure reproduces
without any of this Task's changes, so it is an environment issue (git
commit/push behavior under the test harness here), not a regression. Left as-is.

## Assumptions made (headless, proceeding)

- **`gh pr checks --required --json name,bucket,link`** is the required-check
  read (gh 2.86 supports it). `bucket` buckets: pass/fail/pending/skipping/cancel.
  `cancel` counts as failing (blocks merge). If `--required` yields nothing or
  gh is unavailable, CI state is unknown → `lf status` falls back to plain
  review waiting. Non-required checks never drive the CI owner.
- **CI state is persisted** on the Task PR (new `github_head_sha` +
  `ci_observation` columns) because `lf status` reads stored state
  daemon-lessly and must not make a live GitHub call per task. Reconcile (which
  already shells `gh pr list`) does the read and writes the observation.

## Deferred to PR 2 (the wake)

PR 1 (this branch) ships *truthful CI observation* only. The wake —
`LaunchIntent::CiFix`, `ChildCommandKind::CiFix`, the `ci-fix.yaml` turn,
dedup by `(head_sha, failure_set)`, and the `reconcile_process_liveness`
queue bridge (W2-144 gen 7) — lands in PR 2. See
`scratch/wake-a-waiting-task-into.md`.
</content>
