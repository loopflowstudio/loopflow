# LOO-408 delivery

Remaining: hosted final Linux deletion proof and verified merge. Published installation
is still required for LOO-353; the PR records the next published release as its check.
The accepted design is retained at 796caa84b:scratch/manage-tasks-without-stale-execution.md.

Checks: CI passed the new Linux uncertainty-preservation case; corrected cancellation/cleanup and planning-baseline assertions, then `cargo test --lib ops::pm::task_planning_tests` passed 17 (one fixture entry ignored), fmt/clippy passed; final Linux proxy rerun belongs to CI.
