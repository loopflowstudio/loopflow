# LOO-776 follow-ups: two `lf task run` faults seen while landing #1839

Direction 2026-11-09, Jack Heart: "fix them in this task".

## 1. `lf task run` inside a Desktop Session errors after launching

Seen: `lf task run LOO-776` from an agent in a Desktop pane printed
"A server-bridged terminal relay names its Session (`--session`)"; the run had
already started.

Cause: Desktop's pane script exports `LF_SERVER=1` and `LF_SERVER_STATUS` for
its relay (`GhosttyTerminalView.swift`). The owner removed neither from the
program it runs, so the agent inherited them, and `attach_with_terminal` took
every attach for a server-bridged pane relay that must name its Session.

Fix (done): the owner strips both from its program (`transport.rs`, beside
`LF_TERMINAL_ID`). Test `owned_program_does_not_inherit_the_desktop_pane_relay`
fails without it, passes with it.

## 2. `lf task run` at the last interactive step merged the PR

Seen: Task stood at `demo` (feature graph). `lf task run LOO-776` started after
demo (`select_task_run`: an interactive step counts as done when run from),
`reaches_end` was true, so `start_run_in` approved shipping (`Approval::Route`),
and `land --wait-and-fix` merged #1839. The task-session and wave-operate skills
say plain `lf task run` approves nothing; `TaskGraph::guidance` says only `-b`
and `lf ship` approve. A `land` step in the graph merges whenever reached,
independent of that approval.

Asked "Docs are right" vs "Behavior is right"; answer 2026-11-09: "Behavior is right".

Fix (done): the run approves itself as before. `task.run`'s `running` result
gains `ships`; `lf task run` prints a `ships:` line naming `lf task hold`. The
task-session, task-operate, wave-operate, repo-session, wave-session and
launch-plan skills and `TaskGraph::guidance` now say a plain `lf task run` with
no interactive step left ahead approves Auto-Ship.

## Checks

- `owned_program_does_not_inherit_the_desktop_pane_relay`, macOS: fails without the fix, passes with it.
- `cargo test -p loopflow --lib task_graph_launch`, macOS: 47 pass.
- `cargo test -p loopflow --lib -- engine::task server::wire`, macOS: pass.
- clippy `--all-targets -D warnings`: clean.
- Remaining: gate and CI.
