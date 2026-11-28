Read when the change touches turn or tool hooks, or what they record.

# Turn and tool hooks

A Session launched in a Task's checkout is Task work. Headless (`-b`) there
confines the agent and runs on the native login, like every other launch:
with the real `HOME`, a private `LF_HOME` and that home's `lf server run`,
`lf -b --task <id> -a claude:haiku : '<msg>'` in the Task checkout answers.
Starting a private server resumes every Task with auto on:
abandon old QA Tasks first. Dead AgentProcesses are reaped by
the server's recovery pass, not by `lf session list`.
Interactive (`-i`) launches run in a PTY. A stand-in
`claude` on `PATH` can read `--settings` and run each hook command exactly as
installed, with Claude's callback JSON on stdin; release its turns with marker
files so steers and server changes can happen between them. Run `lf` commands
from the disposable repository: from inside this checkout they resolve
loopflow's Linear planning.

Managed means the launch reports turns, and transport decides it. A Task's
primary conversation (started by `lf checkout <task>`, reopened by
`lf resume --task <id>` or a bound Session's `lf resume`) is a work session
held by `lf holder run`; a Task Graph's headless step is a worker. Both get
`Stop` and `crossSessionInbound: accept` in Claude's `--settings`, and
`LF_PROVIDER_CONTEXT`. Direct `lf -i` and `lf -b` launches are chats and
batches even with `--task`: start, compact, prompt and tool hooks only,
`LF_BARE_STEER`, and Task steers through `LF_STEER_TASK`; a chat's resume
stays in the calling terminal, and a Graph's commit message is a batch. A
stand-in must honor each hook group's `matcher`, or it runs the compact
`SessionStart` hook at startup too. Use a private Machine and create a Task
with `lf task create`, then run `lf checkout <id>` to start its work session.
Driver scripts that delete their marker directory
must do it before waiting on it, or a stale `provider.jsonl` releases a turn
too early. A managed launch
with the server down starts a detached `lf server run` for the private home
before the agent starts: check `lf server status` after launch, not only the
hooks' output. Real OpenCode
answers with no login (its free `opencode/big-pickle` model), so
`lf -i --task <id> -a opencode : '<message>'` in a PTY drives the real plugin;
its turns land in `$HOME/.local/share/opencode/opencode.db` (`session_message`).
The model runs tools in the checkout, so keep it disposable. Pi needs the nvm `bin` on `PATH` and a model;
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
in a Task checkout, and `-b` there. In a
private home with no Linear, `lf task update` needs the full `task_…` ID; the
short identifier resolves through Linear.

To count every `lf` a provider starts, point `LF_BIN` at a `/bin/sh` wrapper
that appends its argv to a file and execs the branch binary; plugins run
`spec.executable`, which comes from `LF_BIN`. A `ps` sampler misses short
processes. An `lf -i --task` conversation stays held after its PTY
closes: stop providers whose cwd is the private Task's checkout, with their
descendants, after every run. Freeze the private server by the PID its `nohup` recorded, never by
`pgrep -f 'lf server run'`, which matches other worktrees' servers.

A Task launch with no server runs `lf install server`, which starts a detached
`lf server run` for the private home (never a launchd job); it exits when idle
or when `$LF_HOME` is removed, or stop it by PID (`lsof -t $LF_HOME/loopflow.db`).
SIGSTOP the server's PID for the slow-server case. Gap lines are in `$LF_HOME/logs/lf.log` for hooks,
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
