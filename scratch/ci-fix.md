Failed head 43667f1dd41ea2c0010619ee1f40ec124bb20e31, run 43529193248,
job 132243626688: checkout adoption read Idle instead of Running at
ops/task/tests.rs:541. The focused test reproduced this locally; its SQL still
wrote the retired tasks.worktree column.

- `cargo test -p loopflow --lib ops::task::tests::`: 28 passed after repair.
- `cargo fmt --check` and `git diff --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
