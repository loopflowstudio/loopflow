# LOO-717 merged verification

PR #1811 merged as `76a5a30125609f19a3974b371d7431d8b208449e`
at 2026-11-07 04:51:52 UTC. `lf sync --manual origin/main` brought this
Task checkout to `c56b38318`; its tree and main's tree are both
`85b19d3d220d311fbaa6ccd07e7d77d5bda87db6`. Checks below run that tree.

The preceding pass delivered the Desktop-owned runtime, one viewer protocol
over SSH and one scheduled retirement sweep. This pass checks the merged
result, not earlier branch test results. The approved shipping steer retained
native/physical-remote demonstration limits and excluded live installation and
scheduler migration.

**Decision: Fix forward / iterate.** The merged runtime passed the checks below,
but the prepared remote-viewer acceptance test exposed a zombie-inventory
false positive. The follow-up filters zombies from the terminal assertion using
the same rule as fixture cleanup. No runtime defect was demonstrated. Publication
of the correction remains with the current Graph's delivery steps.

## Checks

- `cargo build -p loopflow --bin lf` — passed on the merged main tree.
- Built CLI in a temporary `LF_HOME`, inherited `LF_*` cleared and stdin closed — version succeeds; empty `task retire` exits zero; removed `task shell --help` exits 2; private Desktop runtime help succeeds without the GUI.
- `cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored` — two passed, one failed; log `/tmp/loo717-verify-ssh.log`. Twenty real loopback-SSH shell cycles passed (destination/cwd, ordinary `lf`, retained PID/state, no replay, abrupt loss, Close and process/PTY/socket baseline). Authentication/stale-checkout/ended-shell failures passed without local fallback.
- `scripts/test_desktop.sh --filter 'WorkspaceRetentionTests|WorkDestinationTests|DesktopPaneControlTests|SessionsPolishTests|GhosttyShellBlockTests'` — app/FFI build and 105 tests in five suites passed headlessly; log `/tmp/loo717-verify-swift.log`. Covers shell location/remote refusal, exact Close/Undo, workspace/history retention, recovery presentation and actual bundled shell integration output. Display-only tests remain skipped; existing native linker warnings remain in the log.
- `cargo nextest run -p loopflow --lib --test integration --test transport_tests --build-jobs 4 --test-threads 4 -E 'test(ops::task::retirement::) | test(store::sqlite::task_retirement::) | test(transport::lifetime::) | test(transport::viewer_tests::) | test(desktop::shells::) | test(cleanup_schedule_) | test(desktop_shell_) | test(returning_task_viewer_cancels_shell_close_without_another_sweep) | test(viewer_stream_) | test(machine_commands::remote_session_open_returns_an_owner_pinned_terminal_command)'` — 40 passed; log `/tmp/loo717-verify-rust.log`.

These exercise same-execution recovery without replay; twenty local lifecycle
cycles returning owned jobs/sockets to baseline; Close/job settlement; silent
viewer expiry under continuous output; healthy/passive viewers; pipe input,
resize and exit; preview recovery; backpressure; returning viewers canceling
automatic closure between sweeps; completion/reopen/checkout changes; protected
work and draft retention; crash/disconnect absence; cleanup retries; an empty or
interrupted sweep; minute scheduler configuration; removed Task shell API and
GUI-independent Desktop runtime discovery. The two-minute default remains
covered by the existing config assertion at `config.rs:1099`; lifetime checks
here exercise its policy with accelerated intervals.

## Remaining native observations

No rendering environment or separately configured physical remote Machine is
available to this headless run. Native Desktop/Rex interaction, orderly GUI
Quit/relaunch, physical network loss, sleep/logout/reboot and wall-clock
15–16-minute cleanup remain unobserved. Loopback SSH and accelerated retirement
fixtures cannot establish those observations. Follow-up Task `a0dff7`
(`task_a0dff7ea0ee74b38a0f2f4cd82393abb`) links back to LOO-717 and PR #1811 and
captures this verification work. It was saved locally; Linear sync is pending.
Creation started no Graph and requested no installation or schedule registration.

## Concrete repair: remote PTY acceptance counts zombies

Delete — do not maintain: replace the state-blind terminal filter in
`ssh_prepared_viewer_reuses_owner_without_an_extra_pty_and_releases_on_leave`.
Keep rejection of additional live terminals and the existing lifecycle checks;
no runtime, timing, or retry changes are needed.

`ssh_prepared_viewer_reuses_owner_without_an_extra_pty_and_releases_on_leave`
failed at `rust/loopflow/tests/transport/ssh_shells.rs:685`:

```text
extra SSH PTY: [(92404, 92348, "Ss+", "ttys115"), (93264, 92404, "Z", "ttys115")]
```

The live shell and its already-exited child have the same terminal name. The
assertion includes every descendant with a tty and requires every PID to be the
shell PID, ignoring the process-state column it already collected. The fixture's
`assert_baseline` and teardown already exclude `Z` processes. This is a concrete
false positive in the required remote acceptance route, not a demonstrated second
live SSH PTY. The assertion happened before the healthy-idle, resize, leave and
return portions, so those parts did not run in this check.

The live-terminal filter now excludes `Z` process states while retaining rejection
of an additional live terminal relay. No sleeps or retries were added. The earlier
publication reported this test passing; the merged-main run above caught the race.
The correction needs a follow-up PR, keeping #1811 as merged history.

- `cargo test -p loopflow --test transport_tests ssh_shells::ssh_prepared_viewer_reuses_owner_without_an_extra_pty_and_releases_on_leave -- --ignored` — passed after repair (44.13 s), including healthy idle, resize, Leave and unchanged execution on return; log `/tmp/loo717-fix-prepared-viewer.log`.
- `cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored` — all three passed after repair (68.98 s), including twenty lifecycle cycles and process/PTY/socket baseline; log `/tmp/loo717-fix-ssh.log`. Inherited `LF_*` cleared and stdin closed for both runs.
- `cargo fmt --check` and `git diff --check` — passed; Clippy remains with gate before committing.

The repair's test runners and disposable SSH fixtures exited. The existing native
demonstration follow-up and its pending Linear sync are unchanged.

All commands launched by the merged verification pass exited. Its failed SSH test's shell PID and
zombie PID were absent from `ps` after fixture teardown. Successful local/SSH
cycle tests asserted their owned process/socket baseline. No app, provider,
server or watcher remains from this pass, and no candidate was installed or
live scheduler registered.
