Read when the change touches held terminals, `lf holder` or viewers attaching to them.

# Held terminals

Unset `REX_*` along with `LF_*`: an inherited Rex block makes every view
report "open in Rex". A stand-in `claude` on the private `PATH` (answering
`--version`) drives held Task Sessions with `lf resume --task <id>` on a PTY.
Its stdin is lf's first-turn prompt file: drain it with `cat > /dev/null` and
read answers from `/dev/tty`, or the agent answers its own question with the
prompt. A resume of an ended Session pipes no prompt, so that `cat` waits on
the terminal: send Ctrl-D through the view. Emit `ESC]7501;state=blocked:app=claude-code ESC\` to check what
`lf session list --json --all` reads (`availability`) with no view open.

To exercise an install, copy each build aside and point `LF_BIN` at a symlink:
a relay replaced across a `held.sock` revision execs `LF_BIN`. A copy with a
few appended bytes, re-signed with `codesign -f -s -`, runs as a distinct build
(its own holder). A managed launch with the server stopped readies
`com.loopflow.server.<hash>` under launchd for the private home; `launchctl
kickstart -k gui/$UID/<label>` then restarts it the way an install does. Boot
that label out afterwards. A managed launch is held only when a terminal
views it: `lf resume --task … | tail` or `< /dev/null > file` runs unheld
under its own owner. To make server readiness fail without touching launchd,
point `LF_BIN` at a wrapper that exits 1 for `install server` and execs the
branch `lf` otherwise. Find the private server by its store,
`lsof -t $LF_HOME/loopflow.db`; `pgrep -f 'lf server run'` also matches the
Machine's real servers. Kill stand-in agents by their holder parent PID;
holders exit about a minute after their last child. Read the private store
through a copy once the server stops: a read-only open finds no `-shm`.
A view continuing from a PTY position after a server restart writes
`ESC[5n` and holds input until the terminal answers `ESC[0n` (one second at
most); a PTY driver that answers only `ESC[6n` sees that pause. After a build
swap, `lf monitor rollover --dry-run` should list the old holder and its views
as staying; `lf ps` BINARIES names each build with its version.

To see a server whose executable was replaced under it: copy the build to
three paths (`lf`, `client`, `lf-b` with appended bytes), start the server
from `lf` with `LF_BIN` naming that path (`lf install server`, or
`lf server run`), then `rm lf && cp lf-b lf`. Drive the old server with
`client`, the unreplaced copy of its build. The replaced path is another
build: its `lf server status` still reports `ready`, with an
`executable_sha256` that is not its own. A live holder
keeps serving new terminals; stop it (the `lf holder run` under your `bin/`
path) and the next `lf desktop shell <task> --checkout <path> --id <id>
--create` fails naming both builds. `lf install server` from the replaced
path restarts a launchd-run server; nothing restarts a foreground one. A
failed `--create` keeps its `--id`: retry with a new one. The holder the
server started from the new build exits within a minute on its own.

For final bridge reasons, attach through `session.attach` and consume the viewer
frames described in `transport/README.md`. Check one final `0x07` Close before
EOF: `replaced` after `session new --prepare`, `finished` after provider exit,
and `retired` after shell Close or Task cleanup. Reattach by the same Session
only after the replacement starts; assert no extra provider launch or input
replay. Fault the server separately: absent a Close, EOF remains unknown and
recovery retains the same held terminal. Use the existing heartbeat checks for
quiet providers and a lost return path. Desktop status travels over its private
`LF_SERVER_STATUS` Unix stream; inspect readings and actions there, never files.
Mounted presentation still needs a display-capable check.

# Sizing ownership between views

A Python stand-in `claude` that logs `TIOCGWINSZ` on every SIGWINCH and every
input read from `/dev/tty` (raw mode, `ESC[?1004h` to ask for focus reports)
shows what the agent sees: size changes, focus reports and which terminal's
color answer reached it. Log with `os.write` on an `O_APPEND` descriptor and
tag each line with the PID: a SIGWINCH handler that writes to a buffered file
the main loop is also writing crashes the stand-in with a reentrant-call
error, the Session closes and recovery relaunches it. Every Task
conversation killed this way is restarted by the next server's recovery pass,
so the log fills with other stand-ins' lines.

Drive views from Python `pty.fork()` with `TIOCSWINSZ` widths, writing
`ESC[I`/`ESC[O` for focus and answering `ESC[5n`, `ESC[6n` and `OSC 11 ;?`
with a per-view color. `lf session resume <id> [--unfocused]` execs into
`lf session attach --terminal … --session …`: the focus rides in
`LF_RELAY_FOCUSED` (read it with `ps eww`), not in argv, and the relay's env
says `LF_HOME=/private/tmp/…`. A server restart nudges every held terminal
(one column narrower for 0.4 s), viewed or not; a view's focus change while
the server is down reaches the new server once the relay reconnects.
