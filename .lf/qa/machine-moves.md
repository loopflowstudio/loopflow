Read when the change moves, steers or reads a Task across Machines.

# Cross-Machine steering and status over real SSH

Use three private `HOME` and `LF_HOME` pairs with separate clones of one
disposable bare origin. Commit `planning: {provider: git, personal: true}`
and the QA graph before the first Start. Give each clone a Git identity.
On the owner, run `lf server run`, give it a temporary public name with
`lf machine public sandbox-b`, and create a Task with `--run --json`.
Wait for the Graph to reach its interactive step. The two callers' first
`lf task list` discovers the Task and its claim. Run `lf set --global
default_agent claude` in each home first: with a stand-in `claude` on PATH,
`lf task create --run` otherwise stops at "No agent selected". A caller with no
`lf server run` learns later owner changes (a completion) only when a command
exchanges; `lf task sync <id>` there does it at once. For the unowned case,
`lf wave create placed`, `lf --wave placed set machine sandbox-b` and
`lf task create '<words>' --wave placed` on the laptop.

On macOS, stock `/usr/sbin/sshd -D -e -f <config>` can run as your user on
a high port at `127.0.0.1`. Generate throwaway host and client keys with
`ssh-keygen -t ed25519`. Point `HostKey` and `AuthorizedKeysFile` to those
files. Use `StrictModes no`, `UsePAM no`, `PasswordAuthentication no`,
`KbdInteractiveAuthentication no`, `DisableForwarding yes` and
`AllowUsers <your login>`. Use a `ForceCommand` script to enter the owner's
clone and run `/bin/sh -c "$SSH_ORIGINAL_COMMAND"` with its isolated
`HOME`, `LF_HOME`, `LF_BIN` and `PATH`. Put a wrapper on the callers'
PATH that executes `/usr/bin/ssh -F <private config> "$@"`. OpenSSH gets
its default SSH config from the OS account even with a private `HOME`.
The private config names the test `IdentityFile` and
`UserKnownHostsFile`, with a `[127.0.0.1]:<port>` entry for the host key.

Register `ssh://<login>@127.0.0.1:<port>` with `lf machine add` on the
laptop. Leave the sibling without a route. Compare `lf task status <id>
--json` and `lf task status <id> --subscribe --json --select
reading.kind,task.runs_on.machine.id,graph.sentence` with stdin closed.
Send directions with `lf steer <id> '<words>' --json` and the laptop's
Unix socket `task.steer` request. Before reading the Task again on the owner,
watch `steers` via a read-only SQLite connection. Git imports keep the
comment's source and may generate a different steer id on the owner. Check
one row per direction and the `steer_delivered` row per provider Session.

For an offline peer, close only the test SSH master with `ssh -S <socket>
-O exit <route>` and stop the test daemon by PID. A cached master otherwise
keeps the route alive. The master socket lives in `/tmp/lf-ssh-<uid>-<hash>/`,
where `<hash>` is the first 16 hex digits of SHA-256 of the caller's
canonical `LF_HOME` (`/private/tmp/…` on macOS, not `/tmp/…`). Stopping the
sshd listener leaves an open master serving reads, so a status that still says
"read from its server" after the kill came through it. Break and restore the sibling's origin to check a steer saved
locally. Run `lf task sync <id>` on the sibling to send it and watch the
owner import it during its next background exchange.

Stop the private servers, holders and their providers. Check the private
`server.plist` for a launchd label and boot out that label if a provider
hook or Setup registered it during the run.

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
