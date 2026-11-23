Read when QA runs a branch-built `lf`, its server or a private Machine (`LF_HOME`).

# Private Machine

In a shallow checkout the build panics ordering migrations (`CREATE INDEX
… graph_processes …`), and `rust/loopflow-storage/migration-order.txt` lags
main. Deepen with `git fetch --shallow-since=<months ago> origin main`, write
`_migration_order(Path('.'))` from `scripts/test_task_installation.py` to a
file (import with `sys.path` on `scripts/`) and build with
`LOOPFLOW_MIGRATION_ORDER=<file>`.

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
Commands that print "Opening loopflow://…" (`lf desktop view`) leave the
private Machine: the installed app opens the repository under the real `~/.lf`.
Stop every server and holder launched for the walkthrough: a private home's
`lf holder run` outlives its server, and the providers it holds are its direct
children. For input and resize checks without a provider login, hold a real
shell with `lf desktop shell <task> --checkout <path> --id <id> --create` in a PTY.

A private home keeps two SQLite files: `loopflow.db` (state, inputs as
`captured` `session_events` with their newest ended turn) and `history.db`
(`session_history`, observations; `turns`, one row per turn with Task, skill,
Graph, step, model, token and final-message columns). Read them with `?mode=ro` while a writer may be open:
`immutable=1` skips the WAL and shows a stale file. For an upgrade check, record
turns with the installed `lf` (`lf session resume --prepare --json`, then
`lf -b session resume <id> --message …`), copy the home, then open it with the
branch: `lf session history <id>` is empty until `lf machine prune` or the
branch server's start-up pass moves the old rows. To make a history write fail,
hold `BEGIN EXCLUSIVE` on `history.db` from Python. A process's first write
then waits about 5 s (a 2 s busy timeout, retried by
SQLite's WAL path) and logs one `recording gap: history.db: …` line; its later
writes fail at once, at debug. Sample the
branch `lf` itself (`$LF_BIN …&`), not a shell wrapper's PID. Set `LF_BIN` to
the branch binary for every turn: hook settings name `LF_BIN`, else the
installed `lf`, whose hooks write history rows (transitions) into `loopflow.db`.
`lf machine prune` moves them, which is the upgrade skew, not a branch bug.

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
Never open a copy of the Machine's own store with `lf`, a read included:
`lf session list`, `lf session history`, `lf task status|list|show`,
`lf wave status|list|show` and `lf plan list` run recovery first
(`ops/recovery.rs`), which follows the real worktrees and PIDs the copy
records. It starts `lf task run <id>` in the real checkout of any Task whose
run's carrier has exited, and can stop a headless agent the stale copy takes
for orphaned. A copy
(`sqlite3 "file:$HOME/.lf/loopflow.db?mode=ro" ".backup <copy>"`) is for
`sqlite3` only: counts, `EXPLAIN QUERY PLAN`, timing a statement. For
returning-state checks, build the earlier state in a private home with the
installed `lf`, then open that home with the branch. Never seed rows through
internal APIs, keep probe files in the current checkout, and restore its prior
file bytes and index entries.
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

# Comparing served reads between two builds

Make two private Machines with the same script (a repository, a Wave with its
own `config.yaml`, Tasks created with `lf task create '<title>' --wave <wave>`,
a few adopted with `--worktree`), one per binary: the branch build and the
installed `lf` (`whence -p lf`) as the base. Keep `HOME`, `LF_HOME`,
`CODEX_HOME` and `CLAUDE_CONFIG_DIR` under `/tmp/lf-qa-*/<tag>/home`. Build the
branch with `--release` before timing anything: the installed `lf` is a
release build and a debug server is several times slower.

`wave.plan` is `lf wave status --json` as a request; it has no subscription.
The reads that subscribe are `plan.list`, `task.get`, `task.status`,
`session.list`, `session.history`, `repo.tracker`, `task.wait` and the Desktop
views; anything else answers "has no --subscribe". `subscribe` needs
`"sender": null` as `request` does. `skill.list` takes `"path": []`, not null.

A settings or skill file edit reaches the next served read after the Local
refresh, about a second: wait two before asking. Normalize the home path and
`home_<id>` Machine IDs, then diff each step's answer between the two servers.

For CPU, open several connections holding `plan.list` and `task.get`
subscriptions, write through `task.update` (`notes`) a few times a second, and
read `ps -o time= -p <server pid>` before and after; `sample <pid> 5 -file …`
during. Count a function from the call-graph section by summing the top-most
frame of each stack that names it: `grep -c` counts lines, not samples. Rust
v0 symbols are mangled; match `settings14validate_value`, not
`settings::validate_value`. File warnings go to the server's stderr.

Remove and re-add a Task's worktree (`git worktree remove --force`, then
`git worktree add <path> <branch>`) as one of the changes: most recorded Tasks
have no checkout left, and a read that locates a file from the worktree goes
wrong only then. A second repository with nothing under `.lf` in Git, one Task
adopted into a worktree and `lf config set agent …` as its only setting shows
it in `task.get` at `execution.provider` (a subscription frame holds the same
object under `body`).

To see what a later commit changed, keep the earlier branch binary as a third
tag beside base and branch. For a returning Machine, stop the base server,
link a new tag's directory to the base's (`ln -s old ret`) and start the
branch binary there.

Stop the servers by the PID you recorded, after checking its `LF_HOME` with
`ps eww -p <pid>`: other worktrees run their own `lf server run`.

# Comparing a branch with the installed lf

The live store is for `sqlite3` only (above). When it lacks only the branch's
migration (`comm -3` of `ls rust/loopflow-storage/src/migrations` against
`select version from schema_migrations`), snapshot it with
`sqlite3 "file:$HOME/.lf/loopflow.db?mode=ro" "VACUUM INTO '<copy>'"`
(`.backup` restarts on every write and may never finish), then feed the copy
`PRAGMA foreign_keys=OFF; BEGIN; <migration>; COMMIT;` with `.timer on`: the
runner turns foreign keys off too, and with them on a table rebuild fails.
Count before and after, then `pragma foreign_key_check` and `integrity_check`.

For what people see, build the earlier state in a private home with the
released `lf` (`/Applications/Loopflow.app/Contents/MacOS/lf`; `whence -p lf`
can be another worktree's `target/debug/lf`, so check `lf --version`), then run the branch on that home and
alternate the two. Unset `LF_*`, close stdin, set `LF_BIN` to the branch
binary for branch commands only, and put stand-in `claude` and `codex`
scripts first on `PATH`. `lf task create '<words>' --worktree <path> --json`
adopts a worktree; inside it `lf -b graph qa` records a run,
`lf -b default hello` a finished headless Session (the stand-in fails the
turn; the row stays; it must `exit 1` without reading stdin, since `cat` on
`-p` stream-json never ends) and `lf session resume --prepare --json` the Task's
primary. A live taskless conversation is `lf -a claude : 'hello'` under a PTY
driver in a plain worktree, with a stand-in that `exec sleep 300`.
`lf task show <id>` piped prints JSON: `execution.work.sessions[].id` and
`execution.work.graph_processes[].id` are a Task's membership; in SQL, a
Session's Task is `agent_sessions.task_id`, a run's `graph_processes.task_id`
(keyed by `lf_process_id`), a primary a `task_primary_sessions` row, and
`tasks.issue_identifier` holds `lf-<uuid>` for a Task with no tracker. `lf
session resume --prepare --json` prints more than one JSON document; read the
first. `lf session list` shows unfinished Sessions only: a prepared primary lists, a finished `lf -b default` row does not, even with `--headless`; read those in SQL. Its `--task` and `--wave` read `agent_sessions.task_id` (a Session under a Wave names the Wave or one of its Tasks), so they match `lf task show` after a move. `lf task show <id>` answers only from the Task's repository (elsewhere "is not registered", on the installed lf too). A Working-board drag is `lf desktop dashboard move
working-tasks/task:<id> --destination task:<id>`, run from the repository; it
is the only CLI that changes a Task's rank (`lf task move` changes membership
or Machine), and `lf task list` and `lf task show` show the result, `lf task
status --json` carries no rank. A worktree nested inside another Task's checkout (`git worktree add <checkout>/nested`)
is its own Git root, so it belongs to no Task until adopted with `lf task
create --worktree`: launches, bare `lf resume`, `lf task status` ("this checkout
has no Task") and `lf commit` there all answer none, while a plain
subdirectory of the checkout is the Task's. To act as another Task, set
`LF_AS=task:<id>` (a bare id is "invalid Work selector"); a checkout's owner
still wins over it. To move a repository with its checkouts,
keep the worktrees inside it, `mv` it, `git worktree repair`, then run
`lf task list` from the new path (the branch follows the move; released lf
answers "occupied without a Git locator" until `lf config repo --relocate
<repo id>` runs there). Reuse the old path with `git clone` of the origin and
a worktree at the old checkout path. Once every writer has exited, `sqlite3
?mode=ro` cannot open a home's store (no `-shm`); open it read-write.
