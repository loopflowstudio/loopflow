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
