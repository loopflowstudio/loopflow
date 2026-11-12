# LOO-858: headless work without a host

Steer, Jack Heart, 2026-11-12: "Strong bias for deleting `lf session serve`:
the default is to remove it, with its tests, docs and install/rollover
handling, and move Desktop shells to the holder. Keep a piece only if
something concrete still breaks without it, and name that in the PR."

Steer, Jack Heart, 2026-11-12: "Take in two filed Tasks as part of this
cleanup, in this same PR: LOO-830 'Hold Desktop Task shells in the Machine
server's holder' (desktop/shells.rs is the last owner caller your item 2 names
— move it to the holder and delete the owner path) and LOO-838 'Delete the
retired --moment task-updates hook' (…). when this PR lands, complete LOO-830
and LOO-838 alongside LOO-858."

LOO-830 recorded decisions (Jack Heart, Nov 11): "the servers holder should
hold the pty"; "shells in desktop should be managed by the holder as well, but
they should not be managed (and their pty holding should be more generic than
the lf session one)" — a held shell gets no hooks, steers or server launch
check; an lf typed in it stays bare.

## Done

- `os_process::detached`: plain detached launch (setsid, stdin null, fd sweep,
  `LF_LAUNCHER_FD` pipe: Process ID on admission, `released` on let-go).
  `start_lf_session` (Graph launches/`task.run`, landing watchers, ship
  restarts) uses it; output → `logs/detached.log`.
- `transport::delegate`: Work not held → `detached::run` (tails stdout/stderr,
  exits with status or success on release); bare `-b`/piped and headless Graph
  steps run in-process; only Desktop relay surfaces (`LF_SERVER_STATUS`,
  `LF_VIEWER_STREAM`) still start an owner.
- `lets_launcher_go`/`release_launcher` cover held and detached launchers.
- `lf process rollover` deleted (command, alias, `human_session::restore`,
  dead store helpers); inventory module renamed `lf/commands/binaries.rs`.
- LOO-838: `ContextMoment::TaskUpdates` deleted; parse errors in `__` hooks
  exit 0 with the "earlier lf" notice (test `retired_hooks_never_block_their_provider`).

## Remaining (LOO-830 + owner deletion)

- Desktop shells → held terminals: `desktop/shells.rs` creates via
  `session.start` (generic, unmanaged terminal), receipt names a terminal not
  an owner (`desktop_shells.owner` column → migration), shell jobs/Close
  polling move into the holder, retirement (`ops/task/retirement.rs`) ends
  held shells.
- Desktop relay non-work launches (`LF_SERVER_STATUS`/stream) → held.
- Then delete `lf session serve`, `transport::serve`/`start`, `hosted::Local`,
  owner takeover, `Host::Owner` creation; keep `session attach OWNER` only for
  pre-holder owners (see questions.md). Graph fixtures using
  `- cmd: session serve -- python3 …` need another holding command.
- Docs still owner-shaped: transport README lines ~1–190 and viewer-protocol
  prose, `docs/dev/server-first.md` pieces table, `docs/dev/execution.md`,
  `reference-models.md` (`transport_lf_process_id`).

## Checks

- transport_tests: 126/127 then the flake fixed (piped resume has no tty).
- session_lifecycle_tests: 41 pass. task_graph_launch_tests: 53 pass.
- clippy --all-targets clean; documented_commands/cli_discovery pass.
