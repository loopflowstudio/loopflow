Read when the change launches Graphs or Tasks through the server socket.

# Loopflow served launches

Build with `cargo build -p loopflow --bin lf`. Use a disposable Git repository
and separate `HOME` and `LF_HOME`; clear inherited `LF_*`, then set `LF_BIN` to
the absolute branch-built binary. Create the `LF_HOME` directory before starting
that binary's `server run`. `server status` prints JSON with the socket path.
Never fault the installed Machine's store.

Connect over the Unix socket with newline-delimited JSON: send the current
hello from `rust/loopflow-execution/tests/fixtures/dto/server/hello.json`, then requests shaped like
the adjacent `graph_run.json` or `task_run.json`. A local Graph containing
`- cmd: help` needs no provider-backed step; Task checkout may still warm a
primary conversation as described above. For disposable
Tasks, use a local bare Git origin, `planning: {provider: git, personal: true}`
and `graph: qa-normal` in `.lf/config.yaml`. Commit and push those definitions,
then `lf task create --title "Launch QA" --json` creates local planning data.
Create with stdin closed; `--graph` on creation requires `--run`.
Before `task.auto.on`, place the Task with `lf checkout <id> --json`; creating or
binding a prepared Session alone does not place its checkout. `task.auto.on` returns
what `task.run` returns (`state: running`, `graph_process`, `merges: true`); on a
Task whose run is live it turns auto on and returns that run. `task.auto.off`
takes `reason: null` and answers `{issue, was_on}`; nothing but `task.auto.on` turns auto on.

For a checkout without launching a Graph, run `lf wt create qa --json`, take
its `path`, then pass it to `lf task create --worktree <path> --title ... --json`.
Creation returns a receipt containing the Task: use `task.id`.

To exercise external observations, subscribe to `task.get` and inspect
`row.condition.local_progress.dirty`. Edit a tracked file, add an untracked
file, and move the disposable checkout aside and back. Read-only queries of
`store_revisions` before and after establish whether the change arrived without
a store write. A simultaneous selection of `task.title` should send no frame
for those edits. `graph.list` and `config.settings` are request-only reads;
repeat them after editing `.lf/graphs/` or `.lf/config.yaml` externally.

A Swift command-line probe can import the built module with
`swiftc -I swift/.build/debug/Modules probe.swift swift/.build/debug/Loopflow.build/*.o -o probe`.
Encode `ServerClientMessage` values in the probe, then send those exact JSON
lines to the private server. This exercises production encoding, not a window.

For `checkout.commit`, encode `ServedWrite.checkoutCommit` with `push` set
to `false` on an unpublished branch, then send `push: true` with a clean
checkout. Compare `git rev-parse HEAD` and `git ls-remote --heads origin
<branch>` after each request. The second request should publish the existing
SHA. Run the same pair through the CLI using the operation's Swift `argv`.
An installed server can handle the same wire request in its own private home.
Send an unknown operation first and verify that a supported request still
works on that connection. Feed the response to Swift's `ServerMessage(line:)`
with an extra envelope field to exercise decoding across versions.

To delay a report, SIGSTOP the launched command before it reports, leaving its
execution owner running. Check that a live owner waits beyond the unknown grace.
In this disposable home only, make the registry unavailable and restore it;
check recovery both before and after the uncertain reply. Resume the command
and inspect its actual result without launching it again. Find completed direct
Graphs with `lf graph list --runs`, then `lf graph show <ID>`. Stop the private
server and every remaining child after the scenarios.

For Task-run retries, a bare `lf task run` returns at registration; poll
`lf task status <id>` (`Graph <name>: running|stopped at <step>`) rather than
`lf task wait`, which blocks until the Task is done. `lf task show <id> --json`
has the position at `.execution.work.graph.position`, and `lf graph show
<run-id>` lists that attempt's steps. A Graph reads the Task's checkout, so a
Graph file added on `main` is invisible until the branch has it. To make a step
slow or failing, point `LF_BIN` at a wrapper that intercepts that step's argv;
the stand-in step registers no process, so `lf task interrupt` reports it as
"between steps".

For watch recovery, save the baseline's opaque `checkpoint`, close the socket,
then reconnect with the same operation and `select` plus `resume: <checkpoint>`.
An unchanged watch sends its acknowledgement only; send a numeric `checkpoint`
to confirm it is live. Change Work with the CLI while disconnected and check the
complete replacement. Restart the private server to check incarnation changes.
For a terminal `task.wait`, drop after its final reading but before consuming
`ended`; resuming that token must still end. Read field paths from the live
body or schema: `task.get` exposes `task.title`.

`scripts/test_desktop.sh --filter
WorkCLISubscriptionTests/serverReplacementKeepsWorkCurrent` exercises the
production Work model against a real private server: last reading during an
outage, offline edits, continued updates after replacement, and cancellation
during another outage. It is a headless automated check, not a window walkthrough.
