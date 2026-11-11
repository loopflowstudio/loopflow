# LOO-825 publication check, 2026-11-10

The later `gate.md` records passing current checks and native-host acceptance.
The unexplained two-second host-request failures below still block publication.

`lf sync` checkpointed the QA edits as `a73c71151` and found no new base
commits beyond `0766e9cbf`. The diff was reviewed against that base, including
the QA repair. Publication remains blocked; `lf pr publish` has not run.

The affected Rust run reproduced the earlier two-second host-request failures:

- `work_open_tests::cmux_launches_a_fresh_task_workspace_with_its_splits`: 3.912 s; “cmux did not answer within 2s”.
- `work_open_tests::cmux_task_joins_the_group_its_caller_is_in_now`: 3.865 s; “cmux did not report the caller's group; nothing was opened: cmux did not answer within 2s”.
- `work_open_tests::configured_task_layout_shapes_a_launch_without_a_saved_desktop_layout`: 3.858 s; “Rex did not answer within 2s”.

447/450 Rust tests passed. The failure/retry, delayed-readiness, closed-history,
Desktop retained-pane and workspace partial-failure checks passed. The failed
calls use CLI stand-ins, not the person's native host. A separate probe ran the
same shell→Perl acknowledgement shape under `scripts/test_network.py`, four at
a time: the first four completed in 0.332–0.337 s, the next four in
0.063–0.066 s. It did not reproduce or explain the timeout. No timeout was
increased and no later serial pass replaced the failure. Trace the stalled
host child under the parallel run before publishing.

The architecture check found `lf workspace` missing from the public API map.
`docs/dev/reference.md` now includes it; `uv run python
scripts/check_architecture.py` passes all seven categories (27/27 public APIs).

Check command (inherited `LF_*` cleared, stdin closed, candidate on PATH):

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(lf::commands::open::) | test(lf::commands::desktop::) | test(lf::tests::) | test(wave_resolution_matrix::) | test(cli_discovery::) | test(work_open_tests::) | test(dto_fixtures::) | test(documented_commands::) | test(transport::) | test(server::schema::tests::) | test(ops::human_session::) | binary(transport_tests) | binary(lf)'
```

- Rust inventory, formatting and CLI build: passed.
- Affected Rust tests: 447 passed, three failed; the runner did not reach clippy. No Rust code was edited here.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`: passed after the Swift repair; `scratch/publish-clippy.log`.
- Website: 73 passed.
- Headless Swift: build passed; 804 tests ran with four issues across three tests. `attachmentFailureIsStable` exposed release of a pending Session, changing its launch identity before preparation. Retry cleanup now leaves initial pending preparation alone and still retires failed or ended attachments. The other failures were two old expectations that another client's conversation was locally usable; they now require `.elsewhere` and its explanation.
- `scripts/test_desktop.sh --filter 'SessionsStoreTests|WorkspaceSessionLifecycleTests|WorkDestinationTests' -Xswiftc -gnone`: 107 tests passed after the final repair; `scratch/publish-swift-repair.log`.
- Swift multiplatform boundary check: passed.
- `git diff --check`: passed.
- Xcode fallback and remaining native-host acceptance: not run in this attempt; still required before readiness.

Full output: `scratch/publish-checks.log`; phase logs:
`.lf/tmp/gate/run-62812/`. No PR publication, merge, auto-merge or Task completion.
