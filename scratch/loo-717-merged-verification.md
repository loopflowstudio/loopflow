# LOO-717 merged verification

PR #1811 merged as `76a5a30125609f19a3974b371d7431d8b208449e`
at 2026-11-07 04:51:52 UTC. `lf sync --manual origin/main` brought this
Task checkout to `c56b38318`; its tree and main's tree were both
`85b19d3d220d311fbaa6ccd07e7d77d5bda87db6`. The merged checks below ran that tree;
`debf9de042` then committed the test-only repair and this verification record.

The merged change delivers the Desktop-owned runtime, one viewer protocol over
SSH and one scheduled retirement sweep. Jack Heart's November 6 shipping decision,
"ok, lg2m, ship it", covered LOO-717 / #1811 only. Native/physical-remote
demonstration limits remain; installation and live-store/scheduler migration
were not requested. The accepted November 3–6 contract, including the final
Desktop ownership and scheduled cleanup revisions, is retained in
`ca4ca6f75a:scratch/loo-717-shell-lifetimes.md` and Product memory.

The prepared remote-viewer acceptance test exposed a zombie-inventory false
positive. The committed repair filters zombies from the terminal assertion using
the same rule as fixture cleanup; all three SSH checks then passed. No runtime
defect was demonstrated. No further implementation change or product decision
is identified by these findings.

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

Native Desktop/Rex interaction, orderly GUI Quit/relaunch, physical network loss,
sleep/logout/reboot and wall-clock
15–16-minute cleanup remain unobserved. Loopback SSH and accelerated retirement
fixtures cannot establish those observations. Follow-up Task `a0dff7`
(`task_a0dff7ea0ee74b38a0f2f4cd82393abb`) links back to LOO-717 and PR #1811 and
captures this verification work. It was saved locally; Linear sync is pending.
Creation started no Graph and requested no installation or schedule registration.

## Concrete repair: remote PTY acceptance counts zombies

`ssh_prepared_viewer_reuses_owner_without_an_extra_pty_and_releases_on_leave`
originally failed at `rust/loopflow/tests/transport/ssh_shells.rs:685`:

```text
extra SSH PTY: [(92404, 92348, "Ss+", "ttys115"), (93264, 92404, "Z", "ttys115")]
```

The live shell and its already-exited child have the same terminal name. The
old assertion included every descendant with a tty and required every PID to be
the shell PID, ignoring the process-state column it already collected. The fixture's
`assert_baseline` and teardown already exclude `Z` processes. This is a concrete
false positive in the required remote acceptance route, not a demonstrated second
live SSH PTY. The assertion happened before the healthy-idle, resize, leave and
return portions, so those parts did not run in this check.

The live-terminal filter now excludes `Z` process states while retaining rejection
of an additional live terminal relay. No sleeps or retries were added. The earlier
publication reported this test passing; the merged-main run above caught the race.
The correction is committed in `debf9de042`; #1811 remains merged history.

- `cargo test -p loopflow --test transport_tests ssh_shells::ssh_prepared_viewer_reuses_owner_without_an_extra_pty_and_releases_on_leave -- --ignored` — passed after repair (44.13 s), including healthy idle, resize, Leave and unchanged execution on return; log `/tmp/loo717-fix-prepared-viewer.log`.
- `cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored` — all three passed after repair (68.98 s), including twenty lifecycle cycles and process/PTY/socket baseline; log `/tmp/loo717-fix-ssh.log`. Inherited `LF_*` cleared and stdin closed for both runs.
- `cargo fmt --check` and `git diff --check` — passed after repair.
- `git diff --check; lf context` — reconciliation passed; Product memory 15,987/16,000 tokens and 75,725/131,072 bytes; scratch within both limits. Existing SSH results reused; no code changed.

## Remaining delivery

Concept review (November 6): no design change and no open product choice.
The current repair changes only the process inventory assertion. The accepted
actions still map directly to the existing model: Desktop create/attach/Close
uses its execution receipt and shared transport; Session Attach/Resume/Leave
updates Process viewing evidence; `task retire` consumes the SQLite cleanup
queue in one finite Machine sweep. Rex owns its ordinary shells. The Desktop
and transport READMEs already describe these interactions; no usage or skill
revision follows from the test repair.

Normal use retains one remote execution through the prepared viewer. Loss of
that viewer releases its hold without creating a replacement shell; returning
before eligible cleanup reattaches to the same execution and cancels queued
cleanup. Close and history-only Undo remain distinct. A second connected
Session viewer still retains the checkout. The counterexample that must keep
failing is an additional live remote terminal relay: excluding `Z` states does
not exempt that process. A zombie's reported tty is neither live execution nor
a reason to introduce another ownership or retention concept.

No implementation iteration is identified. The next checks remain the gate
below and the separately recorded native observations; this review supplies
no new runtime demonstration or shipping action.

The correction still needs gate and a follow-up PR through the current Graph's
delivery steps. `cargo clippy --all-targets -- -D warnings` remains with gate.
The native demonstration follow-up `a0dff7` and its pending Linear sync remain
separate from this test repair. No acceptance requirement was removed.

All commands launched by the merged verification pass exited. Its failed SSH test's shell PID and
zombie PID were absent from `ps` after fixture teardown. Successful local/SSH
cycle tests asserted their owned process/socket baseline. No app, provider,
server or watcher remains from this pass, and no candidate was installed or
live scheduler registered.
