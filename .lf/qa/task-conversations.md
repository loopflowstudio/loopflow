Read when the change touches a Task's conversations or headless Work.

# Task conversation lifetime

Set up a [private Machine and repository](served-launches.md) with the branch
`lf`. Before starting the server, put a stand-in `claude` first on the
private `PATH` that reads the opening turn and echoes lines from the terminal.
Copy the branch `lf` out of
`target/` before a long walkthrough so a rebuild cannot change the binary
under its owners. Create a disposable Task with `lf task create '<words>'`.
`lf task checkout <task>` warms the Task's conversation detached. Look up its
Session with `lf session list --json`. Bare `lf task retire` runs one pass
over every pending Task. Attach the
Task's Session in a PTY with `lf session resume <session>`, type a line without
Enter as a draft, and close the PTY: the holder keeps it running. Nothing
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

# Headless Work and earlier-build owners

Start the branch's `lf server run` with stdin closed before running
`lf task run <task> <graph> --json`. For a headless step that waits, use
a stand-in `claude` that, when neither stdin nor stdout is a terminal, reads one
line (stdin stays open for steers, so never `cat` it), waits for a marker
file, then prints a stream-json `result` line. A detached launch shows as
`lf task run …` with parent 1 and its stdout on `logs/graph-<id>.log`; `-b` at
a PTY tails unlinked temp files. PTY drivers must drain output continuously:
an undrained PTY blocks the caller on write and looks like a hang. Don't name
a Python driver `pty.py`: it shadows the module it imports.

Workers and batches on real Claude, without lf-world: a plain repository with
a local identity, `default_agent: claude`, a one-line skill and a Graph file
`- <skill>`. Give it a bare `origin` with `main` pushed (`git init --bare`), or
the step finishes and the Graph's push fails, leaving the Task done. A skill
that runs `sleep 90` keeps a worker running long enough to steer. The worker's
ID is in `lf task status <task> --json` under `sessions` (`interactive:
false`). A finished batch (`lf -b : '…'`) is in no listing; read its ID from
`agent_sessions` with Python's `sqlite3` on `file:…/loopflow.db?mode=ro`
(the `sqlite3` CLI fails to open the live store read-only). The Task's primary
conversation waits on Claude's `--add-dir` trust dialog for a fresh `/tmp`
repository; attach with `lf resume --task <task>` and choose "Yes, add it"
(Down, then Enter: Enter alone picks "No, exit"). A primary the server starts
again, after `/exit` or a crash, asks again. To render a raw PTY capture, run
`uv run --with pyte` from the checkout; the `65535u` it shows is Claude's
kitty-keyboard pop, not input.

Breakpoint handoffs on real Claude: a Graph `- step: note`, `- break`,
`- step: echo` with one-line skills on `claude:haiku` parks in about a minute.
`lf task wait <task> --until breakpoint` returns at the park. The reply's
`--reason` shows once in `lf task show <task> --json` under
`comments.comments` with a `loopflow-breakpoint:` marker; the next worker's
first message is in its transcript under
`~/.claude/projects/<cwd with / and . as ->/`. Read `steers` from
`loopflow.db?mode=ro` to see what reached the primary (`breakpoint:<task>`
for handoffs, `update:` for Task news).

An earlier build's Session owner: in a fresh home, before the branch opens it,
run the installed `lf -i -a claude : '<msg>'` (with `LF_BIN` unset) under a PTY
driver that double-forks and `setsid`s, answering `ESC[6n` and `ESC[5n`; 0.16
has no `lf session serve`. Its launcher and agent pids are unfinished
`processes` rows, which the branch's migration copies to `attached_pid` and
`agent_pid` on `agent_sessions`. Write the stand-in's transcript line, then resume with the branch.
SIGSTOP the owner first to check the "still running under an earlier lf"
refusal (SIGCONT afterwards; the pending TERM ends it). The installed
`lf desktop shell <task> --id <id> --create [-- <command>]` in a PTY makes an
earlier-build Desktop shell; its Task needs a checkout (`lf wt create` then
`lf task create --worktree`). Running `open_argv` from
`lf session resume --prepare --json` with `LF_SERVER_STATUS` set to any path
takes the Desktop pane's relayed route.

# Recovery: kills, restarts and reboots

Start the branch's `lf server run` detached (Python `os.fork()`/`os.setsid()`,
stdin `/dev/null`); the recovery pass lives in it. `lf task checkout <task>`
prepares the conversation and the pass starts it (`Conversation: starting`
until its first agent is recorded). `kill -9` the provider (find
it by `pgrep -f 'w/bin/claude --name <task>'`) and poll `lf task status <task> |
grep Conversation` once a second: `died … · starting again`, then `running
(started again …, attempt n)`. To see the `starting` between them (about
150 ms), read `status_line` from `lf task status <task> --json` in a loop. A second kill with no finished turn waits the
backoff; `outlook.at` in `--json` says when, and the start lands on that
second. The stand-in finishes no turns, so the count never resets: five
kills give up with "started 5 times". Type `quit` through `lf resume --task` in
a PTY for exit 0; Ctrl+] steps away and leaves it running. A reboot is SIGKILL
to the server, every `lf holder run` and every provider, then a new server.
Collect the pids with `pgrep -f '^<qa bin>/lf holder run'` (anchored: other
checkouts' fixtures run holders too) and kill them in a loop
(`for p in $(pgrep …) …; do kill -9 $p; done`): zsh does not split an unquoted
pid list, and `eval "kill -9 $P"` runs the second line of a multi-pid list as
a command.
Before each reboot, give every running stand-in a transcript (its
`--session-id` or `--resume` argv, under each checkout's project directory), or
the resumed ones die with exit 1. After the reboot, every conversation should
read `died … (exit 137)`; a `quit` there means an end was settled after the
pass reaped its agent. A shutdown is `kill -TERM -<holder pgid>` and the server.

Replace a binary a server or holder runs with `rm` then `cp`: `cp` over it
breaks its macOS signature and every next exec dies with 137. If the branch's
draft migration changed, use a fresh private home.

For a provider that is missing, start the server from a wrapper whose `PATH`
has no `claude` (starts run with the server's environment, not the CLI's):
a died conversation reads `· starts again HH:MM · last start failed: claude
is not installed on this Machine's PATH`, a Task checked out then reads
`could not start: … · tries again HH:MM`. Restart the server with the full
`PATH` and each starts at the time its line gave.

To skip the 15 minutes of backoff before the five-start give-up, set
`starts_since_turn=4,last_started_at=unixepoch()-3600` on the Session's
`agent_sessions` row, then kill twice.

A returning user: in a fresh `LF_HOME` with the provider stand-in first on
the private `PATH`, run the installed
`/Applications/Loopflow.app/Contents/MacOS/lf server run` detached, then its
`task create` and `task checkout` (the installed lf starts the conversation
itself). Call it by that path: through a symlink of another name, checkout
answers "cannot resolve an executable path for `lf`" and starts nothing. Kill one provider, stop that server, and start the branch's on the
same home: the dead one resumes, the live one keeps its provider. The
installed build's `lf holder run` has no QA path in its argv; find it with
`lsof -c lf | grep <home>` and stop that pid.

The stand-in writes no transcript, so every thread it starts reads as
lost: the next start (the pass's or `lf resume`'s) opens a fresh thread
(`--session-id <new>`) and prints "This conversation has no saved claude
transcript; starting it again from its first request." To check a plain resume (`--resume
<id>`), write a line to `$CLAUDE_CONFIG_DIR/projects/<cwd with / and . as
->/<id>.jsonl` first, taking `<id>` from the provider's argv; to simulate a
crash that lost one, move that file away. Read which thread a start used from
`pgrep -fl w/bin/claude | grep -o -- '--\(resume\|session-id\) [0-9a-f-]*'`;
each fresh start adds one input to the Session. The notice prints on the starting terminal: a person's `lf
resume` shows it, a viewer attaching to the pass's terminal later does not.

A Task run: add `.lf/graphs/qarun.yaml` (`- implement`, `- ship-review`,
`- break`), commit it, and `lf task run <task> qarun`. Its headless step
needs a stand-in `claude` first on both the server's and the CLI's
private `PATH` that, for `-p`, reads one line, waits for a marker file, prints a
stream-json `result` line, and otherwise reads and echoes terminal input. Kill
the carrier's process group (`kill -9 -<pid of lf task run>`) and the step's provider: the
run reads `Run died at implement; the server's next pass carries it again`,
then a new `lf task run <task>` (parent: the server) continues at `implement`.
Killing only the carrier leaves the Graph running; it arrives by itself.
With the server up the death is noticed on the minute tick; a server start
reads at once, after its start-up retention has cleared the dead carrier's
pid (`task_graphs.run_died_at` keeps that it died). The headless stand-in is
`lf --batch`'s child, in its own group: kill it by pid as well.
`lf task run <task> qarun` at the breakpoint continues past it and ends the run.
For the end-to-end `feature` walkthrough (`docs/orchestration.md#where-a-person-takes-part`),
configure different agents for `explore` and the Task conversation, quit the
conversation during `explore`, reopen from **Now** or `lf resume --task <ID>`,
open `scratch/explore.html`, and reply with the non-recommended layout so
`kickoff` receives both the recommendation and the chosen direction.

# A breakpoint review on real providers

Keep the real `HOME` so `claude` and `codex` keep their logins; isolate only
`LF_HOME`, put the branch `lf` first on `PATH` and set `LF_BIN` to it. Give
the workers another model than the primary in `.lf/config.yaml`
(`agent: claude` with `skills: {explore: {agent: claude:sonnet}}`). A provider
out of quota keeps its captured agent across retries: change the setting with
`lf set skills.explore.agent …`, then `lf task interrupt` and `lf task run`.
`lf task create '<words>' --run --graph feature --json` runs explore and parks
at `after-explore` in about two minutes.

Attach with `lf resume --task <id>` under a PTY driver. The `--add-dir` trust
dialog's cursor starts on the exit choice: Enter alone ends Claude with exit 1
(the recovery pass restarts it); send ↓ (`ESC [ B`), then Enter. A driver
that answers no kitty keyboard query leaves `65535u` in the prompt; Ctrl+U
clears it. Esc during a tool call ends the turn without a Stop hook, so the
server does not wake that conversation with a parked handoff; the next typed
prompt delivers it. Read what the primary ran from its transcript,
`~/.claude/projects/-private-tmp-<…>-repo-<task dir>/<provider_thread>.jsonl`,
and what the next worker received from `<checkout>/.lf/prompts/*-steer.md`.
Interrupt and `lf task hold` before the Graph reaches `build`: its `qa` step
resolves to any repository Graph named `qa`. Stop each primary `claude` (a
child of `lf holder run`) by PID, then the holder, then the server.
