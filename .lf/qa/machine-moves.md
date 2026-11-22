Read when the change moves a Task between owned Machines.

# Moving a Task between owned Machines

The same stand-in `ssh` drives `lf machine add mini` and `lf task move <task>
--machine mini` with no real host: it runs its last argument under mini's own
`HOME` and explicit `LF_HOME` (unset, `lf` finds the real `~/.lf`), with mini's
`~/.local/bin/lf` the branch binary and a `~/.gitconfig` identity. A local bare
origin and `planning: {provider: git, personal: true}` are enough; the move's
check clones the repository on mini. A Task whose conversation ran under a stand-in `claude` moves only once that
thread has a transcript: the refusal ("the claude transcript of Session … (<thread>)
is unavailable … nothing moved") names the thread; write one line to
`$CLAUDE_CONFIG_DIR/projects/<checkout with / and . as ->/<thread>.jsonl` for
the `/tmp/…` and `/private/tmp/…` spellings and move again. The host is the
argument before the remote command (`… -- mini <command>`).
To leave a move `sent, not arrived`, swap
mini's `lf` for a wrapper that exits 1 on `session receive` without `--check`,
then read `lf task status`, `lf task list` and `lf machine list` before
restoring it and repeating the move. A wrapper that runs the real receive and
then exits 255 loses the arrival answer. Both homes share the host name, so mini
labels the laptop's claim by its own host name. For mini to reach the laptop
(retries on mini, moves back), route the stand-in by host: when an argument is
`laptop`, run under the laptop's `HOME` and `LF_HOME`; then `lf machine add
laptop` on mini, from outside any clone. Run inside a clone before the first
move, `lf machine add` and `lf config repo --bind` minted mini's own planning
key, and the move's check then refused; start mini's home over. A marker file the
stand-in checks per host (exit 255, "Connection refused") makes either side
unreachable without touching the other's store.

Typed writes travel as `exec 'lf' 'server' 'connect'`, a pipe to mini's
server; `hello` and a request with its `sender` follow on stdin: have the
stand-in `ssh` `tee` its stdin to a file to read them. A plain `tee | bash -c`
pipeline never ends when mini's `lf` exits without answering (its server
cannot start): the stand-in waits on `tee`, which waits on the sender. End
with the remote command instead, as real ssh does: `exec 3<&0`, then
`tee file <&3 > fifo &` (a background job's stdin is otherwise /dev/null),
run the command `< fifo`, kill `tee` and exit with the command's status.
To make mini's server unstartable, boot out its launchd label and point
mini's `lf` at a wrapper that exits 1 on `server run`. A `server run` started
by hand outside launchd can't be replaced when mini's `lf` changes; let
`server connect` start it (it registers `com.loopflow.server.<hash>`, see
mini's `server.plist`) to check an upgrade, and boot that label out after. Mini needs its own
`lf server run` (HOME and LF_HOME pointed at mini's), or `server connect`
registers a supervisor job for it. Swap mini's `~/.local/bin/lf` symlink to the installed
release (no `server connect`), to a wrapper that rewrites the hello's revision
with `sed`, or to one that exits 101 to check each failure the sender names.
Refresh a copied binary with `cp` to a new name, then `mv`: `cp` over a
binary a running server uses invalidates its signature and macOS kills every
later exec with status 137.
