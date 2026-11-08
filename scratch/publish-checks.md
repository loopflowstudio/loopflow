# LOO-804 publication checks — 2026-11-08

- `lf sync`: merged `f61c153eb` cleanly before checks, then `da7e82310` cleanly before publication; launch code unchanged by either merge. Delivery gate checks the final candidate.
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(engine::launch::tests::) | binary(served_reads) | test(documented_commands::)'`: architecture, inventory, formatting, CLI build and 73 website tests passed; 18/19 Rust tests passed. `server_operations_match_their_lf_commands` failed comparing author names, `Jack Heart` versus `Jack`. Runner therefore skipped clippy. Log: `.lf/tmp/gate/run-16839/rust/rust.log`.
- `target/debug/deps/served_reads-0d59b201a60913cf --exact server_operations_match_their_lf_commands --nocapture`, through `scripts/test_network.py`: passed in 28.78s from an empty temporary cwd, isolated HOME and resolved TMPDIR, minimal PATH, inherited LF variables removed, and `LOOPFLOW_TEST_LF` pinned to this checkout's CLI. No production or fixture change was needed. Use this documented isolation for the later gate if ambient identity differs again.
- `cargo clippy --all-targets --jobs 4 -- -D warnings` with inherited LF variables removed and stdin closed: passed. The preceding test link emitted the known large `__eh_frame` warning.
- `git diff --check`: passed. Real socket QA and its limits remain in `scratch/qa.html`.

Hosted CI and landing belong to the following delivery steps. No server or watcher was left running by these checks.

## Delivery gate — 2026-11-08

- Reviewed `origin/main...HEAD` at `a35986a95`, including the clean worktree: launch polling and its tests, troubleshooting, repository QA guidance and scratch artifacts. No in-scope repair needed. `lf sync --manual` found no newer main changes.
- Plan: `engine/launch.rs` reaches its unit tests and `served_reads`; troubleshooting reaches `documented_commands` and website rendering; the runner also selects architecture, inventory, formatting, CLI build and clippy. No Swift, schema or provider changes.
- `TMPDIR=/private/tmp PATH="$PWD/target/debug:$PATH" uv run python scripts/test.py --reuse-passing --rust-filter 'test(engine::launch::tests::) | binary(served_reads) | test(documented_commands::)'` (inherited `LF_*` unset, stdin closed): PASS, all three suites in 262s; 19/19 Rust and 73/73 website tests, architecture, inventory, formatting, CLI build and clippy. No retry or timeout; the prior author-name mismatch did not recur. Logs: `.lf/tmp/gate/run-3054/`. Test linking emitted the known large `__eh_frame` warning.
- `git diff --check`: passed. Only scratch notes changed after checks; production code and check inputs stayed unchanged. The runner exited and its website server stopped.
- Hosted run `42482804202` deferred matrix jobs because the checkpoint PR contains scratch, as `.github/CI.md` specifies; these skips are not passes. Delivery clears scratch before the candidate and merge-queue checks. Existing PR copy retains the socket QA results and late-report retention limit; update its gate status during delivery.
