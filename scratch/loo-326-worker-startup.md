# LOO-326 worker startup diagnosis — 2026-10-02

Jack requested diagnosis and a supported recovery without launching replacement
workers, changing global tmux settings, installing development binaries, or
starting releases. No production source was changed.

## Evidence

- Launcher Execs `8ae8f6a1-b9b4-40e0-8158-878a7839ed77` and
  `633f2a10-adba-457c-8707-0ee3f9f86154` failed after the startup deadline.
  Neither has a child Exec receipt. Their tmux panes no longer exist;
  the server's `remain-on-exit` is off. Original child stderr is unavailable.
- Installed CLI is published 0.12.29, source
  `61d21f88564783a5fc8f63e385ec035f096fc069`; read-only installation preflight
  reports exact schema compatibility and compatible executable references.
- Default tmux 3.7c server PID 72840 holds cwd
  `/Users/jack/src/loopflow.resolve-tasks-from-linear-and`, which no longer exists.
  `lsof -a -p 72840 -d cwd -Fn` identifies that directory; `ls -ld` confirms absence.
- Harmless sessions on that server, with `new-session -c` naming this existing
  checkout, still start with an unresolvable cwd. Both `/bin/sh -c` and `-lc`
  reproduce it. An explicit child `cd` into this checkout restores `/bin/pwd`.
  This rules out login-shell startup as the necessary cause.
- Same-server `lf task status LOO-326 --json` fails with `No such file or
  directory (os error 2)` and `Exec history unavailable: no compatible process
  ledger for this process`. The identical read on a fresh server selected with
  `TMUX_TMPDIR` and `TMUX` unset succeeds, using the same installed CLI and Home.
- The fresh-server read preserves Task `task_d803e583aa4b46be80ffb3624fa5541b`
  and Flow `91f3341e-679d-49ee-a4e5-70460585ee18`, idle at kickoff, unclaimed.

The reproduced launch-environment defect explains failure before Exec admission
and consequently the parent's ten-second timeout. It is strong evidence for the
reported attempts, not recovered stderr from those exact children. Why tmux 3.7c
fails to establish `-c` from this deleted server cwd remains uninvestigated.

## Coordinating-session recovery

Use tmux's ordinary per-process socket-directory selection for the existing
Task's supported continuation command:

```sh
task_tmux_dir=$(mktemp -d /tmp/loo326-tmux.XXXXXX)
env -u TMUX TMUX_TMPDIR="$task_tmux_dir" lf task run LOO-326 --json
lf task status LOO-326 --json
```

Retain that directory while its server or workers are active. This changes no
global tmux settings and does not stop existing sessions. Use `task run`, not
`task restart`, to preserve the captured Flow. The coordinating conversation
must execute the continuation after this diagnostic contribution exits.

This is a workaround with a passing read-only startup proof; worker handoff and
progress to the authored human boundary remain unverified. No worker was launched
here. Temporary diagnostic sessions ran only pwd, installation preflight and
Task status and exited naturally.

## Source comparison and remaining work

Checkout and cached origin/main are `101a19b8c`. Main includes one-Home routing
(#1381), detached tmux clients (#1382), and context scrubbing (#1386), but
`start_tmux_session` still relies on tmux `-c` without establishing cwd inside the
child. Those changes do not directly repair the reproduced deleted-cwd failure.
Do not claim a published upgrade alone fixes it. A durable source repair can
establish the requested cwd in the child before executing lf and preserve startup
stderr; this diagnostic has not implemented or validated that repair.

Review: no raw-store writes, global tmux changes, other-checkout edits, production
installation changes, release activity, or Task/Flow replacement. Swift prevention
remains shipped per supplied steers; the three other LOO-326 faults remain outside
this startup diagnosis.

Check: real tmux read-only A/B probe — existing server fails before Exec admission;
isolated server passes Task status with identical Task/Flow; worker acceptance
belongs to the coordinating continuation.
