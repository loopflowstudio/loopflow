Read when QA runs a branch-built `lf`, its server or a private Machine (`LF_HOME`).

# Private Machine

For live server QA, build `cargo build -p loopflow --bin lf`, then use a fresh
Git repository with a local Git identity and a private `LF_HOME`. Put Desktop
QA homes under a short path such as `/tmp/lf-qa-*`: macOS
`$TMPDIR` can exceed the Desktop Unix socket path limit even though the shared
server uses a short hashed socket. Use that built
binary for every command below. Unset inherited
`LF_*`; close CLI stdin. A private Machine initializes through ordinary commands:
`lf task create --title 'Recover a report' --request-id qa-report --json`.
Run `lf server run` in the foreground and read its socket from `lf server status`;
the socket is hashed under `/tmp`, not inside `LF_HOME`. Speak the documented
newline JSON protocol, disconnect before an answer, then look up
`lf server receipt qa-report --json` before retrying identical content.
Stop every server and holder launched for the walkthrough: a private home's
`lf holder run` outlives its server, and the providers it holds are its direct
children. For input and resize checks without a provider login, hold a real
shell with `lf desktop shell <task> --checkout <path> --id <id> --create` in a PTY.

# Loopflow's local server

Build `cargo build -p loopflow --bin lf` and use that binary for branch QA.
For write scenarios, use a temporary Git repository with a local Git identity,
an isolated `HOME` and `LF_HOME`, and no inherited `LF_*` caller variables.
Set `default_agent: codex` in its `.lf/config.yaml`. Create real records with
`lf task create --title ... --json` and `lf session resume --prepare --json`;
`--prepare` creates connection data without starting a provider. For an upgrade
scenario, prepare a Session with the installed binary before opening the same
state with the branch binary. `lf -b graph <name>` runs a command-only Graph
(`- cmd: help`) in place with no server; run it in a checkout adopted by
`lf task create --worktree <path>` to give that Task Graphs. A `cmd:` step must
name a real `lf` command (`version` is not one). To keep a Graph live, give it
a skill step run by a stand-in `claude` that `cat > /dev/null`: `-p` stream-json
keeps stdin open, so the step runs until you kill the stand-in. macOS has no
`setsid` command; detach with Python (`os.fork()`, `os.setsid()`, `os.execvp`).

Run `lf server run` as a child with closed stdin; get its socket from
`lf server status`. Use the current revision's newline JSON protocol in
`rust/loopflow/src/server/README.md` over that Unix socket. Exercise Swift
encoding with the built `Loopflow` library's `ServerClientMessage` and
`ServerOperation`; this checks live interoperability, not Desktop UI behavior.
After building Desktop, compile a standalone probe importing `Loopflow` with
`swiftc -I swift/.build/debug/Modules scratch/probe.swift swift/.build/debug/Loopflow.build/*.swift.o -o scratch/probe`.
Send its encoded messages to the private socket and decode the live results
with the production DTOs. Plan reads take a positional Wave:
`lf plan board <wave> --all --json`; global `--wave` is context that `--all` ignores.
Check the resulting state through the CLI as well as the socket. Dashboard
queries accept `task:<id>`; hide/show members use that same selector.
`lf desktop layout query:<id>` already prints JSON and takes no `--json` flag.
Save Dashboard slots with `desktop.dashboard.slots`; `desktop.layout.save`
accepts Task, Wave and repository pane layouts. A Task layout holds one Session,
shell and browser pane and includes its `work` key and current `revision`.
Paged Session reads return `entries` and `next`; the unpaged `session.list`
subscription returns an array. Stop and reap every server child,
including on failure; no service installation is needed for this isolated run.

# Loopflow server

Build the branch with `cargo build -p loopflow --bin lf`. Start that binary's
`server run` with an isolated `LF_HOME`, after removing inherited `LF_*` from
its environment. `server status` reports its socket. This leaves the installed
service alone; stop and reap the foreground server when finished.

Drive newline-delimited JSON over that socket, using the hello, subscription,
request and checkpoint shapes in `rust/loopflow/src/server/README.md`.
For returning-state checks, a consistent SQLite backup of the existing store
keeps real Task records without writing the original database. Never seed rows
through internal APIs. Use only reads against that backup, keep probe files in
the current checkout, and restore its prior file bytes and index entries.
Comparison bases accept `parent`, `head` or a resolved commit SHA, not `HEAD`.

For typed subscription events, open `task.get` with `resume: null`, then use
`lf comment <task> '<message>'` to create a real steer. Its `comment` includes
the stored provenance markers: compare the whole entry with
`lf task show <task> --json` at `comments.comments`, not just the input text.
`lf comment` writes; it is not a thread-reading command. Overlapping watches
share a connection sequence but carry the same occurrence ID. A local command
Graph supplies real `graph.step_finished` events; a live
`task.shipped` check requires an actual recorded GitHub merge. Reconnect with a
fresh reading and check that missed comments return as state, without events.
To include an unstarted Task in a dashboard watch, save a query with
`lf desktop dashboard query 'Event walkthrough' 'task:<id>'`; the default
dashboard can have no members for that Task.

Task checkout can also start a background primary conversation when the provider
CLI is installed, even for a command-only Graph. Set `CODEX_HOME` and
`CLAUDE_CONFIG_DIR` inside the private HOME. After the Graph ends, find those
conversations' providers by their disposable checkout cwd and stop them with
their descendants; they are children of the private home's `lf holder run`, so
stopping the foreground server alone leaves them running.
