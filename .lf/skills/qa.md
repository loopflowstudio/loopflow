---
requires: a change or behavior to assess
produces: a change that works for the people who will use it, with what you saw and what is still wrong
action_style: procedural
---
Use the change the way the people it is for will use it, and fix what breaks.

You are its first user. Nobody should open a demo or merge a PR and find it broken
on the first click; if they would, this step failed.

## Use the real thing

Run the real product, not its tests: the built app, the installed CLI, the live
page in a browser, the API over the network, the simulator or device. Drive it
with computer use, a browser, a terminal or whatever reaches it the way a person
would. Internal setters, test-only endpoints, mocks and fixtures do not count.

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
Stop every server and terminal owner launched for the walkthrough.
`lf session serve --terminal -- /bin/sh` prints its owner ID and keeps running;
the first line is readiness, not process exit. Attach with `lf session attach
<owner-id>`. Use a real shell for input and resize checks without provider login.

A packaged Desktop must use its signed bundled `lf` for the QA server too:
signing the bundle changes the CLI hash. Set `LF_DESKTOP` to the QA bundle ID
when using `lf desktop state`; otherwise it addresses the installed app.
Keep the private Machine and provider homes separate from normal Work. Record
window-access failures separately from successful CLI or protocol checks.
If computer use returns `cgWindowNotFound`, inspect the QA bundle with
`lf desktop state --json`; an empty `windows` list establishes no mounted UI.
Do not count a running app process or its background reads as a window test.
Desktop readiness may register a private launchd server. After quitting the QA
app, boot out only the label in that private Machine's `server.plist`; killing
its PID alone lets launchd restart it. Unregister temporary QA app bundles too.

Getting there is your job. Build it, start its services, create accounts and
realistic data, sign in, set up whatever state a real user would already have.
When no route exists, make one. Hand back only what truly needs a person, such
as their own credentials, a physical device, a payment or a production approval,
and say exactly what that left unchecked.

Make the scenario as real as you can reach:

- Arrive the way a person arrives: a fresh install or first visit, then a
  returning user with existing data from before this change.
- Do the main job end to end, then the things people actually do: wrong input,
  empty and large data, going back, interrupting and resuming, a slow or failed
  network, two things at once.
- Check what it leaves behind as well as what it shows: files, rows, messages,
  processes, and what another part of the product now reads.
- Look at it. Read the output, check the screenshot, notice what is confusing,
  slow or ugly, not only what is wrong.

## Loopflow's local server

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

## Launching Loopflow Desktop

The snapshot and quit probe below needs a display; use the headless checks above
when WindowServer is unavailable.

- Build the CLI and local server: `cargo build -p loopflow --bin lf`.
- Build the debug app: `swift build --package-path swift -Xswiftc -gnone`
  (a few seconds when the build cache is warm). Binary: `swift/.build/debug/LoopflowMac`.
- `uv run python scripts/check_desktop_launch.py` launches it with
  `-ui-test-mode empty-workspaces`, waits for the snapshot and the quit, and
  prints the sampled stacks if it hangs. That mode renders fixture data and
  reads no registry, so it runs safely beside the person's live Loopflow.app.
- To look at the frame, call `check_launch(executable, dir)` from that script
  with a directory you keep. `snapshot.png` is written there.
- `scripts/release-loopflow.py` parses no flags. Even `--help` starts a full
  release build.

For a live window on private data, build a world and launch the Dev app on it:
`target/debug/lf-world build one-task /tmp/lf-qa-<name>/world`, then
`uv run python scripts/loopflow-dev.py run-debug --world <world>`. It installs
`~/Applications/Loopflow Dev.app` and stays in the foreground until the app quits.
Run the app's bundled `lf` with the world's `env.json` variables and
`LF_DESKTOP=com.loopflow.mac.dev`; the control socket is under the world's
`LF_HOME`, so `lf desktop state --json` reads only that app.

- Drive it over the socket: `lf desktop ls`, `lf desktop goto 1`,
  `lf desktop run --task <id> '<line>'`. Do not use `lf desktop view` or
  `lf open` there: they open a `loopflow://` URL, and when
  `/Applications/Loopflow.app` carries the same bundle ID macOS starts that app
  on the world instead. It then owns the socket, and its panes run the installed
  `lf`.
- An install that fails with "Third-party inputs changed" means `Cargo.lock`
  changed: `uv run python scripts/third_party.py refresh`, and commit the result.
- To see what a pane inherited, compare `lsof -p <app pid>` pipe addresses with
  those of its children and of the PIDs in the world's `processes` table.
- Quit with `NSRunningApplication(processIdentifier:).terminate()` from a
  `swift` script; both apps answer to one bundle ID. Then end the world's owners
  by PID (check `LF_HOME` in `ps eww`) and boot out the label in its
  `server.plist`.

## Physical iPhone

With the signed iOS app installed and host Tailscale ready, use iPhone Mirroring
through computer use. Resume its existing connection; Command-3 opens Spotlight,
where Loopflow can be opened without fixture arguments. Enter Desktop's Machine
address, connect, search the real Task list and open an existing Session.
An unsent typing check must begin with a visibly empty prompt; remove only the
text inserted for the check. Background with Command-1 and reopen to verify that
the same draft returns as an observer. Leave it passive with the test text removed.
Mirroring establishes physical-device networking and forwarded keyboard input,
not camera scanning, touch-keyboard typing, dictation or cellular recovery.

## Fix and repeat

Repair what breaks within the change's scope, then go through the experience
again from the start. Keep going until it works for the people it is for. A
change to what the product should be is not yours to make; describe it
instead. When asked for a read-only review, report instead of
fixing.

Run tests only where they tell you something the real path cannot reach, and
keep any you add about what a person would notice.

## Loopflow server

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
`CLAUDE_CONFIG_DIR` inside the private HOME. After the Graph ends, find its
`lf session serve --terminal` owners by their disposable checkout cwd and stop
their descendant processes too; stopping the foreground server alone leaves
those conversations running.

## Loopflow served launches

Build with `cargo build -p loopflow --bin lf`. Use a disposable Git repository
and separate `HOME` and `LF_HOME`; clear inherited `LF_*`, then set `LF_BIN` to
the absolute branch-built binary. Start that binary's `server run` there and
read the socket from `server status`. Never fault the installed Machine's store.

Connect over the Unix socket with newline-delimited JSON: send the current
hello from `tests/fixtures/dto/server/hello.json`, then requests shaped like
the adjacent `graph_run.json` or `task_run.json`. A local Graph containing
`- cmd: help` needs no provider-backed step; Task checkout may still warm a
primary conversation as described above. For disposable
Tasks, use a local bare Git origin, `planning: {provider: git, personal: true}`
and `graph: qa-normal` in `.lf/config.yaml`. Commit and push those definitions,
then `lf task create --title "Launch QA" --json` creates local planning data.
Create with stdin closed; `--graph` on creation requires `--run`.
Before `task.ship`, place the Task with `lf checkout <id> --json`; creating or
binding a prepared Session alone does not place its checkout. `task.ship` returns
the shipping Process ID. Inspect its Graph through `lf task status <id> --json`
and pass that Graph's ID to `lf process show`; the shipping carrier is not a Graph.

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
For process liveness, start `lf task create --title ...` with a pipe held open
on stdin. Watch its PID through `process.list`, kill and reap it before sending
input, and expect its removal without a store revision. Repeat through
`lf process list --subscribe --json` with the server stopped.

A Swift command-line probe can import the built module with
`swiftc -I swift/.build/debug/Modules probe.swift swift/.build/debug/Loopflow.build/*.o -o probe`.
Encode `ServerClientMessage` values in the probe, then send those exact JSON
lines to the private server. This exercises production encoding, not a window.

To delay a report, SIGSTOP the launched command before it reports, leaving its
execution owner running. Check that a live owner waits beyond the unknown grace.
In this disposable home only, make the registry unavailable and restore it;
check recovery both before and after the uncertain reply. Resume the command
and inspect its actual result without launching it again. Find completed direct
Graphs with `lf graph list --processes`, then `lf process show <ID>`; the live
process list cannot find a finished run. Stop the private server and every
remaining child after the scenarios.

For Task-run retries, a bare `lf task run` returns at registration; poll
`lf task status <id>` (`Graph <name>: running|stopped at <step>`) rather than
`lf task wait`, which blocks until the Task is done. `lf task show <id> --json`
has the position at `.execution.work.graph.position`, and `lf process show
<graph>` lists each attempt's steps. A Graph reads the Task's checkout, so a
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

## Keep what you learned

How to launch and drive this product belongs in this skill. When you had to work
out a step it does not describe, or a step it describes no longer works, write
the working version here with `lf skill customize qa`, replacing what was wrong.

Show what you did in `scratch/qa.html`: each scenario, what you did, what you saw
with screenshots or transcripts, and what you fixed. Finish with what works now,
what you could not reach and why, and what is still wrong, worst first. Do not
publish, land or choose the next Graph step from here.

## Loopflow Desktop

For Desktop in a headless run, follow `swift/README.md`'s Testing section:
`scripts/test_desktop.sh --filter '<affected suites>'` builds the app and CLI,
then runs model and production-view checks with WindowServer access denied.
Unset inherited `LF_*` and close stdin as described in `scripts/testing.md`.
`DesktopHeadlessTests/workStates` checks loading, unavailable, empty and selected
Work and taps the production navigation button. Report these as automated
checks, not a live-window walkthrough. Hosted window checks remain optional;
record what was not seen. With a display, `uv run python scripts/loopflow-dev.py
run-debug` builds and launches the development app; use `lf desktop state --json`
and the commands in `docs/surfaces.md` to inspect and drive it.

Use a private `LF_HOME` and an explicit branch `LF_BIN` for experiments. Clear
inherited `LF_*` first; isolate `HOME` and the repository too. Never attach the
Session running this QA step.

For headless relay checks, `lf resume --prepare --json` creates connection data
without launching a provider; repeating it keeps the same Session. Start the
branch's `lf server run` in the private Machine and parse `lf server status` as
JSON (its output is compact). Wait for `ready` before starting a relay, so its
initial readiness request does not register a background service. Stop and reap
the private server and every relay afterward.

A probe compiling `ServerConnection.swift` against real relay publications can
check mailbox delivery and cancellation without WindowServer. Label this as a
component probe: it does not establish mounted panes, hidden-pane rendering,
focus, copy/Find, or Move here interaction. Keep display diagnostics optional in
headless runs; record those limits and leave the full Swift suite and Xcode
fallback build to gate. See `swift/README.md` for the supported checks.

## Bare terminal launches

A bare `lf : "<prompt>"` on a PTY runs the provider as `lf`'s child and
the terminal's foreground group leader; `ps -o pid,ppid,pgid,tpgid,tty`
shows it. Drive Ctrl+Z/`fg` from `zsh -f -i` on the PTY, not bare `lf`.
A stand-in `claude` reads its first turn from stdin only when stdin is not a
terminal (`[ -t 0 ] || first=$(cat)`), then keys from `/dev/tty`; on resume
stdin is the terminal. Pi retitles itself `pi`: read its argv from
`processes.command`. A Session the installed `lf` made can be reopened by
the branch only when the branch's migrations are a superset; otherwise it
refuses with "a newer lf migrated this store".

## Codex launch settings

Also isolate `CODEX_HOME`; set `cli_auth_credentials_store = "file"` in its
disposable `config.toml` to keep probes away from the account's keychain.
Start the branch's `lf -i -a codex : "Reply only with QA_OK"` in a PTY. A PTY
driver must answer Codex's cursor-position query (`ESC[6n`, reply `ESC[1;1R`).
Save the terminal transcript, including sign-in or startup failures.

The private store's `agent_sessions.provider_thread` and joined
`processes.endpoint` identify the launched provider. Read them without changing
rows. Connect a WebSocket over that Unix socket with URI `ws://localhost`, send
`initialize` with `clientInfo` and `capabilities.experimentalApi: true`, then
`initialized`. Real Codex supports `thread/resume` with only `threadId` to read
the tier, and `thread/settings/update` to change `serviceTier`. Reattach through
`lf --setting codex.service_tier=default resume SESSION`; compare the thread,
endpoint, provider PID and tier before and after. Stop and reap private children.

Unauthenticated Codex can create, fork and configure threads through this API,
but its terminal stops at sign-in. Label these provider API checks; they do not
establish authenticated replies or typed `/new`, `/fork` and `/fast` behavior.
Use different names for a headless Graph and its skill so Graph resolution does
not treat the step as a recursive Graph call.

## Raw conversations and transcript readings

A plain `claude` lists as raw only with no `lf` or provider ancestor, and a
shell this step starts has one. Run it under a PTY driver that double-forks and
calls `setsid`, so launchd is its parent; keep the real `HOME`, since raw
discovery reads the account's own `~/.claude/projects`, and give lf a private
`LF_HOME`. Strip `LF_*` and `CLAUDE_CODE*` from its environment. Claude's
folder-trust dialog in a fresh directory defaults to "No, exit": send Down,
then Enter. It writes no transcript until the first message, so the
conversation lists only after one.

`lf <read> --subscribe` from the CLI reads in its own process, not through the
server (`lf server status` shows no client). To check what Desktop sees,
subscribe over the server socket with a `desktop` hello and timestamp each
`reading`; compare its frame times with the transcript rows' timestamps.

A bare `lf -a claude : "<prompt>"` in that private `LF_HOME` with no accounts
and the real `HOME` runs on the ambient Claude login, so its Stop-hook and
exit rows can be read from the private store. Unset `REX_*` too, or the run
records the agent's own Rex pane. Read the store with
`sqlite3 'file:<db>?immutable=1'` once no writer is open; `-readonly` fails
when the WAL files are gone. `~/.claude/sessions/<pid>.json` names each live
Claude process's conversation. Run a second `claude` and an lf run in the same
directory to check pairing. Codex's first-run trust prompt writes into the real
`~/.codex/config.toml`; quit it unless that write is acceptable.

Plain `pi` and `opencode` need no trust answer and list the same way. A
headless `lf -b -a pi : '<prompt>'` (or `opencode`) in the raw process's
directory checks pairing against an lf-written conversation and writes
`usage` rows with `cost_usd`. OpenCode's default headless model may fail
upstream; name one that answered interactively, `-a opencode:opencode/<model>`.
`pi --session <file>` resumes a recorded Pi conversation raw. To learn
which filesystem events a provider home really sends, a throwaway ignored
test holding a `notify` watcher on it shows them; FSEvents sent none for
OpenCode's `opencode.db-wal` writes.

## Turn and tool hooks

A Session launched in a Task's checkout is Task work. Headless (`-b`) there
confines the agent and needs a managed login (`claude=<email>`), which a private
home cannot get without real OAuth (`--import` checks the token against
Anthropic). To drive real headless Task runs anyway, run a throwaway
`rust/loopflow/examples/*.rs` that calls `upsert_provider_account` the way
`tests/session_steers.rs`'s `connect_fixture_account` does, against the private
store, then delete it. Put `tests/support/mod.rs`'s `FAKE_SECURITY` first on
`PATH` as `security` and set `USER`: signing the login in goes through the
Keychain. Never run `lf account` there: it checks the token with Anthropic and
marks the fixture login "needs a login"; register it again if it did. A
stand-in's usage-limit result cools the only login for 15 minutes, so the
step waits for capacity rather than stopping. Starting a private server
resumes every Task whose Auto-Ship was approved: abandon old QA Tasks first.
Do not repeat this with release 0.15.3: its
launch from a private home tries to write a Claude Keychain item. Dead
AgentProcesses are reaped by `lf process prune`, not by `lf session list`.
Otherwise launch interactively (`-i`) in a PTY. `lf account connect claude --api-key` with a placeholder
`ANTHROPIC_API_KEY` satisfies account selection for that launch. A stand-in
`claude` on `PATH` can read `--settings` and run each hook command exactly as
installed, with Claude's callback JSON on stdin; release its turns with marker
files so steers and server changes can happen between them. Run `lf` commands
from the disposable repository: from inside this checkout they resolve
loopflow's Linear planning.

Managed is decided by the command: `--task` or `--wave` gets turn hooks and
the server check, on a launch or a resume (`lf -i --task <id> resume`,
`lf session resume <session> --task <id>`, and the `open_argv` that
`lf session resume --prepare --json --task <id>` returns for Desktop). Plain
`lf -b <skill>` gets the start and compact hooks only; plain `lf -i` and
`lf -i resume <session>` keep turn hooks without the check, so they never
start a stopped server. Driver scripts that delete their marker directory
must do it before waiting on it, or a stale `provider.jsonl` releases a turn
too early. A managed launch
with the server down registers it before the agent starts: check
`launchctl list | grep <label>` after launch, not only the hooks' output. Real OpenCode
answers with no login (its free `opencode/big-pickle` model), so
`lf -i --task <id> -a opencode : '<message>'` in a PTY drives the real plugin;
its turns land in `$HOME/.local/share/opencode/opencode.db` (`session_message`).
The model runs tools in the checkout, so keep it disposable and stop the
`opencode serve` child afterwards. Pi needs the nvm `bin` on `PATH` and a model;
a private home has no key. Both reach a scripted local model instead: a small
OpenAI-compatible HTTP server (streamed `chat/completions`, scripted `bash`
calls for Pi, `shell` for OpenCode 2.x, then a reply naming what it saw) named
in `$HOME/.pi/agent/models.json` (`api: openai-completions`, plus
`defaultProvider`/`defaultModel` in `settings.json`) and in
`$HOME/.config/opencode/opencode.json` (`npm: @ai-sdk/openai-compatible`).
Logging each request body shows whether steers and the context block reached
the model. `lf -i --task <id> -a pi` runs native Pi; `lf -b --task <id> -a pi`
runs Pi under lf's RPC driver. Pi's `/compact` refuses a small session; give
the scripted tool a few thousand lines of output first, and type the command
and Enter separately or its autocomplete picks another command. Pi truncates
tool output, so even `seq 1 60000` can leave a real-model session "too small".
Write each typed line to Pi's PTY in one write, not
per character, and drain output between writes of more than a few KB.

Real providers on their existing logins: keep the real `HOME` with a private
`LF_HOME`. Plain `lf -b` and `lf -i` launch there, as does `lf -i --task <id>`
in a Task checkout; only `-b` in a Task checkout asks for a managed login. In a
private home with no Linear, `lf task update` needs the full `task_…` ID; the
short identifier resolves through Linear.

To count every `lf` a provider starts, point `LF_BIN` at a `/bin/sh` wrapper
that appends its argv to a file and execs the branch binary; plugins run
`spec.executable`, which comes from `LF_BIN`. A `ps` sampler misses short
processes. Each `lf -i` leaves a `session serve` owner after its PTY closes:
stop owners whose argv names the private Task, with their descendants, after
every run. Freeze the private server by the PID its `nohup` recorded, never by
`pgrep -f 'lf server run'`, which matches other worktrees' servers.

A Task launch with no server runs `lf install server`, which registers
`com.loopflow.server.<hash>` for the private home; boot it out with the label in
`$LF_HOME/server.plist` to stop the server, and again when finished. SIGSTOP the
server's PID for the slow-server case. Gap lines are in `$LF_HOME/logs/lf.log` for hooks,
`$LF_HOME/logs/pi-extension.log` for Pi and
`$LF_HOME/runs/<xx>/<run>/opencode.stderr.log` for OpenCode;
delivery evidence is `lf session steers <id> --json`; a recovered gap shows as a
`store-gap` row in `lf session history <id>`. Read the store with
`sqlite3 "file:…/loopflow.db?immutable=1"` when it has no `-shm`.
A private home that applied an earlier draft of the branch's migration refuses
to open (checksum mismatch), as does one that applied the branch's migration
before a merge put main's newer migration ahead of it ("migration history or
schema differs"). Start a fresh one, seeded by the installed `lf` when the
upgrade path matters.

Boundary query plans: copy the Machine's store with
`sqlite3 -readonly ~/.lf/loopflow.db ".backup <copy>"`, compare
`schema_migrations` with `store/migrations/`, and apply only the branch's
missing migration SQL to the copy with `sqlite3`. Never open the copy with
`lf`: recovery acts on the real PIDs and worktrees it records. Dump `TRACED`
from `boundary_statements_never_scan_tables_that_grow_with_use` to JSON (a
temporary line, reverted), then `EXPLAIN QUERY PLAN` and time each statement
on the copy inside a rolled-back savepoint, before and after `ANALYZE`.

Config loaded inside a write transaction: in a private home, replace
`$LF_HOME/config.yaml` with a FIFO. At each open, before writing the file's
contents, try `sqlite3 -cmd '.timeout 0' <db> 'BEGIN IMMEDIATE; ROLLBACK;'`;
a failure means lf holds the write lock while it reads config. Stop the
feeder with a flag file, not `kill`: a surviving feeder interleaves writes and
leaves lf blocked on an open with no writer. In this shell `which lf` names a
wrapper function; use `whence -p lf` for the installed binary.

Read changes on a large private repository: `lf task create` takes seconds
each, so create hundreds in parallel batches with `--request-id` (twelve at a
time held up), claiming and completing some. For a "before" binary, run
`--version` on sibling worktrees' `target/debug/lf` and pick a build that
descends from the branch's base with no `store/migrations/` diff; it reads the
same private store. Compare `jq -S` output of both binaries, then time them in
interleaved rounds, since other worktrees' builds make the Machine's load swing.
`lf server status` prints JSON without `--json`.

## Waking idle conversations

Real turns need real logins: keep the account's `HOME` (Claude's Keychain login
and `~/.codex`) and isolate only `LF_HOME`, with the branch `lf` first on `PATH`.
Interactive `-i` launches need no lf account. `lf checkout <task>` itself starts
the Task's primary Claude conversation, which runs one `task-session` turn and
goes idle: that alone is a managed idle conversation. Start Codex with
`lf --task <task> -i -a codex : "Reply only with QA_OK"` in a PTY; its first turn
needs no typing. A conversation is a wake candidate once
`session_activity.yielded` is 1 and `processes.inbox` (Claude) or
`processes.endpoint` (Codex) is set. Then `lf comment <task> 'Reply only with X'`
and read the reply: Claude's in
`~/.claude/projects/<cwd with / as ->/<thread>.jsonl` (a `queue-operation`
then a `user` cross-session message), Codex's through its app-server
(`thread/turns/list` with `itemsView: full`). A Codex client may sit on an
update dialog; its app-server still runs the turn. Kill the private server's
PID (find it by `LF_HOME` in `ps -E`) to check news recorded while it was down.
`ps` matches `__context-block` in Codex app-server argv: those are hook
declarations, not processes.

## Native conversation opening

Use the private homes above for native servers too. Start Rex with
`rex server run --listen unix://<root>/rex.sock --state-dir <root>/rex-state
--config <root>/empty.lua`; create an empty Lua file first. Every Rex client
command takes `--autostart=false -S unix://<root>/rex.sock`. Never launch a second
Rex app: its preferences share the installed app's cached route. A headless Rex
server can attach and capture real terminals; focus correctly fails without a
capable display client.

Open with `lf open --tab --host rex --server unix://<root>/rex.sock --json`.
Use `rex api call session.list_blocks '{"session_id":"…"}'`, then
`rex block capture -s <session> -b <block>` to read the terminal. To check the
direct default inside Rex, create a `session.new_window` whose terminal command
is `["<branch-lf>","open","--task","<task>"]`; it must stay in that block.

For Herdr, isolate `XDG_CONFIG_HOME` and `XDG_STATE_HOME`, start
`herdr --session qa server`, and set `HERDR_SOCKET_PATH` to
`<config>/herdr/sessions/qa/herdr.sock` in clients. `herdr pane list` and
`herdr pane read <pane> --source visible --format text` show the real destination.
Two simultaneous opens of one prepared Session should create one attachment.
`herdr workspace create --cwd <repo>` makes the calling pane; to act as an agent
tool call from it, set `HERDR_ENV=1`, `HERDR_PANE_ID`, `HERDR_WORKSPACE_ID` and
`LF_AGENT_CALLER` to `{"session_id":…}` of a Session prepared with
`lf session resume --prepare --json` (process IDs may be any UUIDs). Opening
then takes the sibling-tab path. In zsh, kill a PID list with `kill ${=pids}`:
an unquoted variable is not word-split.

To exercise failed-start recovery with real Codex, exit its private sign-in
screen with Ctrl+C, temporarily put invalid TOML in the private Codex config,
and reopen that Session. Capture the retained error tab, restore the config,
then repeat the exact opening command. Check the Session and provider-thread
identity, and capture again after startup settles. Attachment at the sign-in
screen does not establish authenticated conversation behavior. Stop native
servers and their terminal descendants, including detached owners recorded in
the private store.

## Held terminals

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

## Task conversation lifetime

`cargo build -p loopflow --bin lf-world`, then `lf-world build one-task <dir>`
writes a private Machine whose `bin/claude` stand-in echoes each line; its
printed environment holds `LF_HOME`, the provider homes and `PATH`. Under an
isolated `HOME`, give that home a Git `user.email` first or the build fails at
claiming. Copy `lf` and `lf-world` out of `target/` before a long walkthrough so
a rebuild can't change the binary under its owners. `lf task checkout <task>`
warms the Task's conversation detached; `lf session list --json` maps it to its
Session. Bare `lf task retire` runs one pass over every pending Task. Attach the
Task's Session in a PTY with `lf session resume <session>`, type a line without
Enter as a draft, and close the PTY: the owner keeps running detached. Nothing
schedules checkout cleanup in a private home; call `lf task retire <task>` to
exercise deletion (the installed `com.loopflow.cleanup` job runs once a minute).
After `lf task complete`, run `lf task retire <task>` once fifteen minutes have
passed without a viewer and live work has settled. That pass signals the verified
conversation owner directly, even without a checkout; a held Task
conversation ends on the server's notice instead. `lf task abandon` ends only
unviewed conversations; one still viewed then ends at the next
`lf task retire <task>` after its view leaves.

A new conversation's stand-in reads its opening turn from stdin, then the
terminal, as Claude does, but writes no transcript: before resuming one after
its provider ends, write any line to
`$CLAUDE_CONFIG_DIR/projects/<project>/<id>.jsonl` (the ID follows `started`). For a done Task without a checkout, prepare a repository
conversation (`lf session resume --prepare --json` in the repository), complete
a checkout-less Task, then `lf session bind <session> --task <task>`. Binding
claims the Task, which reads `git config user.email` in the repository: with an
isolated `HOME`, set a repository-local identity first. A PTY driver waiting for
text can match the repainted saved screen before the new provider reads input;
wait for text only the new provider prints.

## Rented Machines without a provider

Renting runs the provider bridge as `uv run … python -c <bridge> <provider> <op>`
and reaches the sandbox with `ssh`, both from `PATH`. Stand-ins on a private
`PATH` drive `lf machine ondemand rent`, `lf task move --machine` and
`lf machine end` with no account or cloud: `uv` answers `create` with a unique
`id` and an SSH `route` (`qa@sandbox<n>.invalid`), `get` with the sandbox or
`null`, and `delete` with `null`; `ssh` takes its last argument as the remote
command and runs it under a separate `HOME`/`LF_HOME` per sandbox, with the
branch `lf` on that `PATH`. Exit 0 for the `install.sh` command and print
`0\n0\n` for the `pgrep` holdings script. Each sandbox needs its own home: one
Machine identity behind two rentals fails `rentals.machine_id`. Set any
placeholder `DAYTONA_API_KEY` and an SSH key in the private `HOME`; stub
`ssh-keygen -R`, which would edit the real `known_hosts`. To land a write between
a preflight read and its transaction, serve a repository's `.lf/config.yaml`
through a FIFO and act from another repository at the first open.

## Test-only changes

When the change moves or cuts tests, the people it is for are developers
running them. Run each filter exactly as `rust/loopflow/tests/README.md`
prints it, with `LF_*`/`REX_*` unset and stdin closed. Then break the
production decision on purpose: one `sed` on the line the rows claim to
check, run that family's filter, `git checkout -- <file>`. Do one for each
family, plus one Clap → ops wiring break (e.g. drop `worktree` in
`lf/commands/task_create.rs`) run against the kept full-Machine filter.
A break no test notices is a missing row; add it, then repeat the break.
Edit only from one shell, one cargo command at a time, so no other build
compiles the mutated source.
