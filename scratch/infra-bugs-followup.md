# Task checkout adaptability

Accepted 2026-10-14: Jack Heart accepted PR #1555 after walkthrough review and
requested queue then ship. The delivery covers checkout naming/restoration,
missing-directory reconciliation, and sequential Task PR continuation. The disclosed completed-Task
launch/reopen boundary remains outside this delivery; publication can reopen a
completed Task, but launching it is not redesigned here. Provider switching,
historical Wave-directory migration and builtin Flow/Workflow review are excluded.

## Accepted behavior — implemented

- Use shared `WorktreeSegment` normalization, including single-word and long
  names. Restore/create the checkout before selecting explicit agent configuration.
- Missing checkouts leave Task, PR and landing intent intact. Reconciliation
  continues with available siblings without inventing completion or recreating
  work. Other filesystem/provider errors remain errors.
- Continued publication works in ordinary and Task checkouts after squash merges,
  including pruned remote branches and unobserved merges. Keep staged, unstaged,
  committed and already-pushed follow-up work. Only an exact delivered remote head
  may be replaced under a lease; concurrent remote work survives.
- Atomically archive the settled PR when inserting its successor. Freeze earlier
  PR placement/history, clear old completion intent, reopen completed Tasks, and
  keep retries on the new PR. An old landing cannot complete the new delivery;
  failed insertion rolls back archival and reopening.
- Submit creates missing PRs without an extra flag; the published flag remains
  accepted. Repeated landing on a merged Task PR ignores scratch-only finishing
  notes. Follow-up Tasks remain advice for distinct outcomes, never a prerequisite.

## Implementation and review

Removed the Task-only name parser, settled-PR publication refusal and internal
create-PR option. Removed `ops::land::ensure_pr`: preparation directly reads or
creates the PR after its existing availability check. Existing landing tests
cover both paths.
Flattened `ops::sync::restart_landed_branch` to use a concrete replay target and
reuse its branch lookup; `push_with_upstream_if_needed_locked` also reuses its
branch lookup. The delivered-head proof, exact lease and pushed follow-up ancestry
remain intact.

Gate found that the delivered-head proof was tautological when pushing a
divergent default branch: its tracking head was compared with itself. The leased
replacement now also requires the integrated base in HEAD's ancestry. The
existing rejected-push regression and all continuation scenarios pass together.
Updated the work-skill prompt golden. The completed-Task fixture now verifies
completion despite the expected primary-checkout cleanup error before proving
that publication reopens it; production launch/completion behavior is unchanged.

Reconciliation on 2026-10-14 found no remaining implementation mismatch within
the accepted scope. Review confirmed that successor insertion owns archival,
completion-intent clearing and reopening in one transaction; failed insertion
rolls them back. Missing-directory checks retain saved intent and propagate other
filesystem errors. Fresh `origin/main` is `92b88800ec` (v0.13.15), already merged
into this branch; no newer upstream change alters this approach. No Wave is bound.

## Delivery evidence and remaining work

Historical reproduction and verification details are in
`af5547315a:scratch/infra-bugs-followup.md`. Fresh Task creation plus explicit-agent
launch reproduced missing Git metadata before provider startup; deleted checkouts
reproduced reconciliation failures. The fixes passed disposable CLI proofs.

PR #1555 retains the prior check results: 70 checkout checks, 111 delivery/migration/
docs checks, 28 startup/release-lock/scheduled-release checks, 70 release tests and
three final landing regressions, plus build, Clippy and formatting. Proof uses real
Git repositories and the built CLI with simulated GitHub responses. It does not
claim an installed Fantasia release replay or live GitHub merge lifecycle.

Gate acceptance is complete. PR #1555's copy preserves the useful findings,
accepted launch boundary, final checks and evidence limits. Remaining delivery:
clear added scratch through landing preparation, publish the candidate, then queue
and observe landing. Hosted full-matrix CI remains deferred until scratch cleanup;
the current PR's skipped matrix is not a full pass. No further product decision is
needed. No unrelated Wave files were changed.

Checks: `scripts/test.py --reuse-passing` stopped at its 60s resource-inventory timeout before product checks; direct equivalent checks used stable Rust 1.99.0, four workers and the documented network boundary. Materialized `cargo nextest run --all --no-fail-fast --build-jobs 4 --test-threads 4 -E 'not test(data_clones::)'`: 2601 passed, 3 failed, 22 skipped; after repairs, the same materialized runner with `-E 'binary(golden_prompt) | binary(task_pr_range_tests) | binary(task_pr_authority_tests) | binary(release_tests) | test(commit_tests::) | test(sync_tests::) | test(land_tests::) | test(agent_startup_tests::) | test(release_lock_tests::) | test(scheduled_release_tests::) | test(ops::sync::)'`: 214/214 passed; materialized `cargo test -p loopflow --test integration --jobs 4 data_clones:: -- --test-threads 4`: 4/4 passed; final `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `scripts/check_architecture.py`, `scripts/check_rust_test_inventory.py` and `git diff --check`: passed; `website/dev.py test`: 75 passed; isolated `pytest python/tests/test_loopflow_skill_alignment.py`: 4 passed. Logs: `/tmp/lf-gate-1555*.log`. Hosted full matrix and environment-specific opt-in checks remain unclaimed.
