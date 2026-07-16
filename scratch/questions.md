# Open questions

## Pre-existing `land_tests` failures (not caused by this branch)

`latest_land_disposition_wins_before_merge` and `lf_ops_land_leaves_worktree_in_place`
in `rust/loopflow/tests/land_tests.rs` fail on the baseline (verified by stashing
this branch's changes and running the tests on clean HEAD). Both fail with
"ambient Task Session ts_... is not registered" — a test-harness registration
issue unrelated to session body resolution. Out of scope for this work.
