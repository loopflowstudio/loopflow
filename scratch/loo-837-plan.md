# LOO-837 plan

Status: implemented. One rule in `rust/loopflow/src/ops/task.rs`: `select_stack_parent` (read; also used by `checkout --explain`) and `record_stack_parent` (push if origin lacks the branch or is behind, never force; record an unpublished `task_prs` row when the parent has none). Store `stack_task_placement` no longer requires a GitHub PR, only an unsettled one.

Why kickoff-placed Tasks had no row: `place_task_in` (`store/sqlite/children.rs`) inserts the placement PR only when `publication.is_some()`, i.e. when a GitHub PR already exists for the branch. Every fresh placement behaves this way, not only `feature`-graph ones; 821–828 had rows because they had published. Placement is unchanged; recording a `Working` row at every placement would change board and snapshot phases far outside this Task. Say so in the PR.

Checks:
- `cargo test -p loopflow --lib stack`: pass
- `cargo test -p loopflow --test integration sync_tests::stack`: pass (new `stacking_on_an_unpushed_parent_pushes_its_branch_and_sync_follows_it`)
- `cargo test -p loopflow --test global_commands task_checkout_explain_reuses`: pass
- `documented_commands::`: pass; clippy `--all-targets`: clean
- Gate owns the affected-suite run.
