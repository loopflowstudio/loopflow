# CI repair

Failed head: `cfc4293cf3297ef0cab32c1cbab48aec11dae35b`.
Job: https://github.com/loopflowstudio/loopflow/actions/runs/43340231344/job/131608282624

The planning-recovery fixture failed before its read: `error in trigger store_revision_tasks_update after drop column: no such column: NEW.ship_lf_process_id`.
The same shard passed on base `78bb9f55641af07d8877b6b02dc12f5bb2aa19af`.

- `cargo test -p loopflow --lib lf::commands::waves::tests::planning_failure_keeps_the_stored_workflow_readable -- --exact`: reproduced the CI error before repair.
- `cargo +stable test -p loopflow --lib lf::commands::waves::tests::planning_failure_keeps_the_stored_workflow_readable -- --exact`: passed after repair on Rust 1.100.0 with inherited LF variables cleared and DEV/TEST_DEBUG=0; CI runs the full shard after publication.
- `cargo +stable fmt --all -- --check`: passed.
- `cargo +stable clippy --all-targets -- -D warnings`: passed on Rust 1.100.0; Cargo emitted existing unused-dependency manifest warnings for chrono-tz, rpassword and tokio-stream.
