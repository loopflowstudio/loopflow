Read when the change touches how CLI, Desktop or the phone reach a Task that runs on another Machine (routed reads, saves, shells, Sessions, `lf open --task|--machine`).

# A Task on another Machine, by hand

Two private Machines on one Mac, everything through the CLI (see
[machine moves](machine-moves.md) for the stand-in `ssh`). Keep the world
under a short `/tmp/lf-qa-*` path. The pieces that worked:

- `bin/ssh` on the laptop's `PATH`: log each call, exit 255 with
  "Connection refused" when `bin/ssh.offline` exists, else run the last
  argument under `env -i HOME=<mini> PATH=<mini>/bin:/usr/bin:/bin bash -c`.
  Mini's `bin/lf` is a symlink to `.local/bin/lf`, a wrapper exporting mini's
  `LF_HOME` and `LF_BIN`.
- Mini's `bin/claude` prints `provider on mini in $PWD` and echoes lines read
  from `/dev/tty`; that is the Session check. Mini needs
  `lf set --global default_agent claude`: harness detection does not count the
  stand-in, and a routed `lf open --task` on a Task with no Session refuses
  "No agent selected" (even with `-a claude`) until the owner has a preference.
- Laptop: `lf machine add mini`, `lf task create '<words>'`, `lf task move <id>
  --machine mini`; arrival makes mini's checkout. A laptop checkout made first
  (`lf task checkout`) and edited after the move is the stale copy to look for.
- Run both `lf server run`s yourself and record their PIDs. After every
  rebuild restart both: mini's `server connect` from the new binary refuses
  ("another executable still answers"), and the laptop reports only "mini did
  not answer within 10s". Never run mini's `server connect` by hand while its
  server is down: it registers `com.loopflow.server.<hash>` for mini's home
  (`launchctl bootout gui/$UID/<label>` from `remote/store/server.plist`).
- Speak the socket with a tiny newline-JSON client (hello, then
  `request`/`subscribe` envelopes with `select`, `request_id`/`resume` and
  `sender` all present). Print whole lines: `task.get` frames run past 1500
  characters. `task.get` takes `select: null`; a bad select yields `{}` and
  later readings are suppressed as identical.
- Checks that tell: `lf task files|file|diff|changes|save` from the laptop
  (mini's bytes, never the stale copy's), `lf desktop shell <id> --id x
  --create -- /bin/sh -c 'hostname; pwd'` under `script -q /dev/null`, `lf open
  --task <id> --here` and both `--machine mini` orders in a PTY, and a
  `task.get` subscription across a move (laptop checkout, null, mini's).
- mini's served reads answer from its Git observation: an edit shows in
  `changes`/`diff` a few seconds later, not at once.
- Detach from a bridged viewer is `Ctrl+]`; typing `quit` to the stand-in
  makes it exit, which prints the owner's leave line. Both must name `lf open
  …`, never `lf resume <id>`.
- A PTY driver in Python must not be named `pty.py` (it shadows the stdlib
  module); run it with `python3 -I`.
- `task_checkouts` names mini's checkout ~100 ms before Git makes it: sample
  the row and `[ -d … ]` in a loop during `lf --machine mini task checkout`
  to see the window a files watch must survive.

# Moving a Task with a Session, for Desktop

A Task whose Graph never ran opens on the Task's own **Not started** page, so
the panes need a Session: on the laptop, `lf set --global default_agent
claude` (and on mini), a laptop `bin/claude` stand-in, `lf task checkout`,
then `lf session resume --task <id> --prepare --json`. The move then refuses
until the thread has a transcript; write one line to
`<laptop>/.claude/projects/<checkout with / and . as ->/<thread>.jsonl` for
the `/tmp/…` and `/private/tmp/…` spellings (thread id is in the refusal).
A dirty laptop checkout gets committed and pushed by the move; `git stash`
the stale edit around the move to keep it local.

# Desktop with the screen locked

Build the isolated bundle with the QA hook ([desktop](desktop.md), "Driving
the app with the screen locked"), launch it on the laptop's home with `HOME`
pointing at the laptop home, which needs a `src/` directory or the window says
"Repositories did not load" and has no plan rows (Files then refuses "Where
this Task runs is not known yet"). Then:

```sh
open -g -b <bundle id> 'loopflow://repo/task/<id>'
LF_DESKTOP=<bundle id> lf --task <id> desktop do details page=files
LF_DESKTOP=<bundle id> lf --task <id> desktop do files path=notes.txt
LF_DESKTOP=<bundle id> lf --task <id> desktop do details page=diff
```

and `snap` through the hook. A wrapper that runs the CLI under `env -i` must pass `LF_DESKTOP` through, or
`desktop do` addresses `com.loopflow.mac` and fails. `desktop focus -p
session` is the visit that attaches a moved Session's pane; before it the
pane can say "Session location unavailable". Files shows `mini <checkout>` above the file;
the sidebar row of a moved Task must not show the stale copy's "Blocked · Task
has uncommitted work". A Task whose Graph never ran shows the Task's own
**Not started** page; use a Task with a Session for the panes. The hook cannot
see SwiftUI buttons (`press Retry` answers "not found"), so a `click` on
Retry cannot be confirmed except by the next snap. Quit with `osascript -e
'quit app id "<bundle id>"'`, `lsregister -u` and delete the bundle, then
remove `QAHook.swift` and its `LoopflowApp.swift` line.

# Desktop panes across a move (Slice 6 run)

- Launch the bundle without a URL (`open -n -g --env … "$APP"`), then
  `open -g -b <bundle id> 'loopflow://repo/task/<id>'`. A URL passed to the
  first `open -n` went to `/Applications/Loopflow.app`, which started with the
  private `LF_HOME`; stop that one by PID.
- A pane is ready for input only when `lf desktop state --json` shows its
  `attachment: "usable"`; `desktop send` refuses while it says `opening`.
  `desktop send` never submits: follow it with `desktop send-key -p session enter`.
- To see what a bridged relay tells Desktop, bind a Unix listener in a short
  `/tmp` path and run `lf open --task <id> --session <sid> --here` in a PTY with
  `LF_SERVER_STATUS` pointing at it: a working relay writes `connected`, then
  `native_ready: true`. Kill a PTY relay with care: it takes the viewer from
  Desktop.
- Relaunching without `defaults delete` restores a second window stuck on
  "Updating…" that shares the repository's workspaces. Keep it for a move: it
  is how two windows reading a Task differently show up. A crash leaves no
  `.ips`; `/usr/bin/log show --predicate 'processID == <pid>'` (zsh's `log` is a
  builtin) has the DisplayCycle stack, and `sample <pid> 3` shows a spin.
- The Session header's `plus.bubble` is **Reset session**, not New conversation.
  New conversation lives in the "…" menu and the ⌘/ Task actions popover; the
  hook reaches neither while locked. A link to an unstarted mini Task starts
  its conversation on mini, so the Not started page with **New session** is gone
  by the time the window draws.
- A Session with no viewer ends after two minutes (idle owners); its pane then
  says "Session ended." Refresh or focus reopens it on mini.

# Reset session and New conversation on a Task mini runs (Slice 7 run)

- A Task that only ever ran on mini: `lf task create`, `lf task move <id>
  --machine mini` while unstarted, then `lf task run <id>` from the laptop
  (it runs on mini; with the `qa` Graph it stops at demo with a Session).
  The laptop's plan row then says "No Task Graph has run": that is the case
  where Desktop must still open the Session pane, not **Not started**.
- Hook clicks do not reach a pane strip's controls (Reset session's
  `plus.bubble`, the Brief/Diff/Files tabs): nothing changes and no request
  reaches the server. Send Desktop's Reset over the socket instead, exactly as
  `RegistryQuery.resetSession` encodes it: `session.new {directory,
  request_id, id, task: <laptop Task id>, agent}`. Then open its `open_argv` in
  a PTY: the stand-in prints a new `--session-id` on mini.
- New conversation on a remote Task is a shell on the owner:
  `lf desktop shell <task id> --id <tab> --create -- lf --interactive --task
  <identifier> : '<prompt>'` (Desktop adds `--bootstrap`); close it with
  `--close --started-on <owner machine id>`.
- A served `session.list` needs `headless`, `after` and `limit` beside `task`.
- `creation.get` for a routed `session.new` answers `null` on the laptop; the
  receipt is in mini's store (ask mini's socket).

# Peer readings and routed receipts (Slice 8 run)

- Request shapes that cost a retry: `task.get`, `task.status` and
  `task.detail` take `{directory, issue}` (not `id`); `task.files.watch` takes
  only `{issue}`; served `session.list` wants `headless: false`, not null. A
  `subscribe` envelope carries `resume: null` where a request has
  `request_id`; with `request_id` instead the server says it "has no operation
  `task.get`".
- Two private Machines on one Mac share Task IDs but not repository paths, so
  the localized keys to look at are `row.task.repo` (the laptop's path), the
  owner labels (`row.runtime.owner.machine.label`,
  `ownership.owner.machine.label`, `workspace.machine_label`: all `mini`, never
  mini's hostname) and every `open_argv` (`lf open --task … --session …`).
- A lost Reset: send `session.new` with a `request_id`, then `lf server
  receipt <request> --task <id> --json` on the laptop. Without `--task` it
  prints `null`. Run the receipt's `result.open_argv` in a PTY: the stand-in
  prints the Reset's new `--session-id` on mini.
- With the screen locked, a Dev bundle opened at `loopflow://repo/task/<id>`
  still attaches the Session pane: `lf desktop state --json` shows
  `attachment: "usable"`, and `desktop send` / `send-key enter` / `capture -p
  session` round-trip through mini. A Reset leaves an `lf holder run` under
  mini's `LF_HOME`; stop it by PID with the servers.
