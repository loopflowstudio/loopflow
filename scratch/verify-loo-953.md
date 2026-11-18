# LOO-953 after merge

PR #1974 merged 2026-11-18T19:18:00Z as `7bb879aff4`. This checkout's HEAD
(`29acd8dea1`) has the same tree as `origin/main` (`git diff --stat HEAD
origin/main` prints nothing), so the build below is main's.

## CI for 7bb879aff4

| event | workflow | result | run |
| --- | --- | --- | --- |
| merge_group | CI | success | https://github.com/loopflowstudio/loopflow/actions/runs/44377135607 |
| push | CI | success | https://github.com/loopflowstudio/loopflow/actions/runs/44379029846 |
| push | macOS on main | failure | https://github.com/loopflowstudio/loopflow/actions/runs/44379029858 |
| workflow_run | Packages (main) | in progress when read | 44380382718 |

Push `CI` ran merge-proof, task-installation, rust-build, rust-lint and
e2e-smoke; rust-test, python-test, migration-check and architecture-check were
skipped there (they ran in the merge group).

`macOS on main` failed in `macos / swift-test` on one test; both
macos-process-test jobs and loopflow-ui-test passed:

    ✘ Test ignoredWritesFromReadsNeverScheduleAnotherRead() recorded an issue at
      TaskFilesTests.swift:684:9: Expectation failed:
      (readings.withLock { $0 } → 5) == (settled → 4)
    ✘ Test run with 786 tests in 97 suites failed after 76.202 seconds with 1 issue.

The test counts FSEvents-driven reads across fixed one-second sleeps. #1974
changes neither `TaskFilesTests.swift` nor `TaskFilesStore`, and the same job
passed on the PR's last pull_request run (44373983901). Filed as LOO-975.
The previous `macOS on main` run (40b1d88c19, #1975) also failed, on a
different cause: `loopflow-terminal/build.rs` could not download
`libghostty-vt-a60e9e2-lf7-aarch64-apple-darwin.tar.gz`.

## Commands, isolated home

`HOME=/tmp/lfv953/home LF_HOME=/tmp/lfv953/lf target/debug/lf …`, inherited
`LF_*` unset, stdin `/dev/null`.

- `lf process list`, `lf ps`, `lf top`, `lf monitor`: `error: skill or graph
  not found: <name>`, exit 2.
- `lf session history list`: `Error: Session list was not found`, exit 1 (the
  page of commands is gone; `list` reads as a Session name).
- `lf graph list --processes`: `error: unexpected argument '--processes' found`.
- `lf graph list --runs`: exit 0, empty on a fresh home.
- `lf machine prune --help`: "Apply store retention: end runs and agents nobody
  saw exit, expire Session history past 90 days and reclaim the space".
- `lf machine doctor` on the fresh home: `migrations: applied
  process_rows_on_owners / known process_rows_on_owners`, `ok strays every
  process carrying a Loopflow identity belongs to a live Session or run`.
- Stray: `LF_HOME=/tmp/lfv953/lf LF_STEER_SESSION=session_gone node -e
  'setTimeout(()=>{},120000)' &` then doctor printed `warn strays 1 process(es)
  carry a Loopflow identity no live Session or run claims: 82977 (node)`. The
  process was left running by doctor and killed by hand.
- The same plant under `/bin/sleep` and `/usr/bin/perl` was not reported: macOS
  hides the environment of system binaries (`ps eww -p` shows none either), so
  the scan cannot see their variables. Agents and lf are not system binaries.

Schema of the fresh store: `agent_sessions` has `agent_pid`,
`agent_os_started_at`, `agent_ended_at`, `agent_exit_code`, `agent_endpoint`,
`agent_inbox`, `attachment_token`, `attached_lf_process_id`, `attached_pid`,
`attached_os_started_at`, `attached_terminal_id`; `task_graphs` has `run_pid`,
`run_os_started_at`. Two tables still reference `processes`:
`relocated_task_processes` (leaving, LOO-965) and `steer_batches` (leaving,
LOO-954).

## Named tests on the merged tree

Run with `LF_*` unset and `< /dev/null`; all passed.

- `cargo test -p loopflow-storage --lib run_owners_carry_their_process_lifecycle_after_upgrade`
  (a store written by released 0.16.0 SQL upgrades onto the owner columns): ok.
- `cargo test -p loopflow --test integration a_killed_graph_run_lists_as_stopped_and_a_live_one_as_current`: ok.
- `cargo test -p loopflow --lib -- a_step_that_dies_before_writing_its_end_ends_as_its_graph_saw_it
  task_ship_restarts_on_the_next_read_after_its_process_dies
  a_process_carrying_a_session_that_nothing_claims_is_a_stray`: 3 passed.

## Not checked here

- A real `lf task run -a claude`, kill, `lf graph list --runs` walk with a
  provider agent. The killed-run test above drives the same commands with a
  stand-in agent.
- A live Graph launched by released 0.16.0 across `lf install`. The PR states
  it ends at its next step's schema check; nothing here ran a released lf.
- Desktop views. `swift-test` on the PR ran them; the push run failed on the
  unrelated test above before reporting a full pass.

## Outcome

Follow-up. Every claim checked holds on main. The one red run for the merge
SHA is a sleep-timed Swift test outside this Task's change, filed as LOO-975.
