# Troubleshooting

Something stopped. Three questions, in order: what survived, what went wrong,
what to run next. Every command here is complete and runnable as written.

## What survives

A crash, a reboot or a closed terminal loses less than it looks like.

```bash
lf resume                       # return to this scope’s conversation
```

| Survives | Where it lives | Pick it up with |
|---|---|---|
| Conversations, interactive and headless | `~/.lf/loopflow.db`, `~/.lf/history.db` and the provider's own history | `lf resume SESSION`, `lf resume --task <ID>` |
| A Task's worktree, branch, PR and thread | Beside your repository, on the code host | `lf task run <ID>` |
| A stopped Graph | Its run and step results, as history | `lf task runs <ID>` |
| Uncommitted files | The worktree; prune never removes them | `lf wt list` |

A stopped Graph never resumes. `lf task run` starts a fresh one in the same
checkout, with the Task's thread and its open PR, at the step the last one
stopped on; the steps before it stay done.

## What went wrong

Read before you relaunch anything.

```bash
lf doctor                                    # installation, store and stray processes
lf task status <ID>                          # the Task's execution and the reason for it
lf graph show <RUN_ID>                       # each step, when it ran and how it exited
lf session history show <SESSION_ID> --final # the agent's last word in that conversation
lf ps                                        # what is running right now
```

`lf task status` reports execution separately from the Task's own status:
starting, running, stalled, blocked, idle or unknown, with its reason. Unknown
liveness stays unknown; lf never guesses a process is dead.

## What to run next

| You see | Run |
|---|---|
| A question waiting for you | `lf resume --task <ID>` |
| The run is going the wrong way | `lf steer <ID> "address the latest feedback"` |
| It must stop now | `lf task interrupt <ID>` |
| It stopped and should go again | `lf task run <ID> --reason "provider credentials repaired"` |
| It stopped, and an earlier step's work must be redone | `lf task run <ID> --from <step>` |

## Doctor reports a failure

```bash
lf doctor --json
```

Doctor checks installation selection, store compatibility and stray processes
separately. It reads storage without initializing it or applying migrations, so
an incompatible store still reports which Sessions and runs it can read.

Binary freshness compares against locally cached `origin/main`, without
fetching. That reference may be stale, and merged source may be newer than the
stable release. `lf install` installs that stable release; Doctor
itself installs nothing.

Contention reads the last day of `logs/lf.log`: `database is locked` failures
and any write that held the store lock for 3 s or more, grouped by the lf
binary that logged them, with the holder's pids and the function that held the
lock. Lines from binaries older than this check group as `unidentified (older lf)`.

## Desktop reopened, but the Session will not accept typing

Select the same Task and use **Refresh session** in the Session pane's toolbar.
The surviving conversation keeps its draft and pending requests, and the selected
pane regains focus when input is ready. Switching Tasks is unnecessary.

While **Opening… Input paused** or **Reconnecting…** appears, the screen may be
readable before typing is available. If another view has control, choose
**Take back** only when you want to move input here; Refresh does not take it.
If the provider has ended, use the offered resume action to continue its history.
Refresh never repeats a submitted message or restarts the Task's Graph.

If Refresh leaves input paused, follow the pane's connection error and recovery
action, then check the server below. See [Session recovery](sessions.md#when-desktop-loses-its-connection)
for what survives a Desktop exit versus a provider exit.

## Desktop cannot connect

```bash
lf server status   # readiness, socket, server instance and connection diagnosis
lf doctor          # selected installation and store health
```

**Opening… Input paused** means Desktop is waiting for attachment and native
input readiness. Its last available screen stays readable; without a saved
screen the same pane stays empty until output arrives.
**Reconnecting…** means a connection was lost and Desktop is trying the same
running conversation. Its agent is still working: agents run in a holder
process that a server restart leaves alone. Neither status changes the terminal's dimensions. Keep the pane
open; its screen and local reading actions remain available. If automatic retries
end, **Retry connection** reconnects without replaying input or restarting the provider.
A lost connection-status channel also pauses input; the terminal relay keeps
running and restores its status automatically when that channel returns.
A read-only pane's **Move here** requests control explicitly; a lost connection
alone never takes it from another viewer.

If the diagnosis reports disabled background access, enable Loopflow in macOS
System Settings, or enable its user service on Linux. Retry and status do not
change that setting. Use `lf install` for an installation/build mismatch; do not
start a second foreground server. A server outage alone is no reason to relaunch
the Task's Graph. See [Connect clients](server.md).

## A steer arrived late, or a turn is missing from history

**Symptom:** the agent kept working past a steer, or `lf session history`
shows a gap where a turn should be.

```bash
lf server status                      # is the server listening, and on this build?
grep 'recording gap' ~/.lf/logs/lf.log | tail
grep 'recording gap' ~/.lf/logs/pi-extension.log | tail   # Pi conversations
```

Hooks at each prompt and turn end ask the Machine server to record the turn.
When the server is stopped, restarting or slow, the hook gives up after two
seconds and the conversation continues without it; `logs/lf.log` has one
`recording gap` line for that conversation until the server answers again.
OpenCode and Pi report from inside the provider instead of a hook process, and
write that line to their own log: `logs/pi-extension.log`, or
`opencode.stderr.log` beside the Session's capture. The missed turns show in
the Session's history as one `store-gap` entry, read as unknown rather than
still running.

Steers do not wait on the server: each hook reads them from the store. One a
hook claimed and never printed shows `pending` again in
`lf session steers <session>` after fifteen seconds and is printed at the next
boundary; a steer can arrive twice this way, and is never dropped.

Nothing needs relaunching. If `lf server status` reports no server or a
mismatched build, run `lf install`.

A conversation started before an upgrade keeps the hooks its build wrote. One
that names a command or value this lf no longer has (`--moment task-updates`,
say) prints `lf: ignoring …, a hook from an earlier lf; restart the Session to
refresh its hooks` and lets the turn go on. Quit the agent and `lf resume` it
to get current hooks.

A run started with plain `lf <skill>` or `lf graph`, by you or by an agent,
reads its steers from the store after a tool call or at a prompt, with no
server: one sitting idle at its prompt hears nothing until you type.

## A Task run stops with "Loopflow server is not running"

```bash
lf install server                     # register and start the Machine's server
lf server status
```

Agents started by a Task or Wave command (`lf task run`, `--task`, `--wave`,
a Task or Wave conversation) and Desktop shells start only once the Machine
server answers, including headless runs, because their turns and steers go
through it. The server does not host a headless run: once started, it keeps
going if the server stops. A launch that finds none runs
`lf install server` itself; the message appears when that did not bring one
up, and carries what the install said. On another Machine, run the two
commands there: `lf --machine <machine> install server`. Plain `lf`,
`lf <skill>` and `lf graph` run without a server, in a Task's checkout too.

Conversations already running in a holder keep running while the server is
away, and their views reconnect when it returns.

## Several `lf holder run` processes

One per installed build that still has an open Task or Wave conversation.
Each exits a minute after its last conversation ends; ending those
conversations, not killing the holder, is how to retire one. Killing a holder
ends every agent and Desktop shell it holds. An `lf <skill>` typed at a
terminal or in a Desktop shell has no holder: its agent is a child of that
terminal. An `lf session serve` process is a conversation an earlier build
opened; it ends when its agent quits, and no current build starts one.
`lf resume <ID>` restarts it: lf ends the old process, waits for it to exit
and continues from provider history, held for Task or Wave work and in your
terminal otherwise. If lf cannot verify that
process or it does not exit, nothing starts and the message names its pid;
quit the agent there, or end that pid, and resume again.

## A creation has no confirmed answer

Desktop retains submitted Task creations and new Session threads across app
restart. Reconnect to the original Machine and let it look up the receipt;
changing repositories does not change the saved destination. An unavailable
receipt lookup leaves the submission unresolved.

For a CLI submission with a saved request ID:

```bash
lf server receipt REQUEST_ID --json  # read the recorded creation; starts nothing
lf task status TASK_ID --json        # inspect any Task the receipt identifies
```

An `admitted` or `started` receipt is complete: the Task exists, and an
`admitted` Task's conversation sets it up. Open it with
`lf resume --task TASK_ID`; while the Task's Graph has never run, opening runs
setup again. A `run_requested` receipt means a `-b` launch is unconfirmed. Inspect the Task's Graphs and processes before starting
anything; a lost answer does not establish a failed launch. See
[creation recovery](server.md#recover-a-submitted-creation).

## Setup failed in a new Task's conversation

The conversation prints the step and its error, for example a `git worktree add`
failure, and the Task's position reads `Setup stopped: checkout failed: …` in
`lf task status` and Desktop. Fix the cause and reopen it:

```bash
lf resume --task TASK_ID   # runs setup again
```

The agent starts once the checkout exists, even if the Graph step failed.

## A launch reports `outcome_unknown`

After starting a Task from a connected client, check whether its Graph started
before trying again:

```bash
lf task status <ID> --json
lf task runs <ID>
```

`outcome_unknown` means the launch may have taken effect but did not report a
usable result. If the command exits, the server stops waiting. If it cannot
determine whether the command is still running, it continues checking for a
bounded time before returning the error. Access to the Machine's records can
recover during that wait.

This error does not stop or repeat the launched work. Leave a running Graph
alone. If status is unavailable, use `lf doctor` and check again once reads work.
Retry only after inspecting the affected work; for launches other than Tasks,
inspect the operation's result and `lf ps` first.

For a Graph launched directly, `lf graph list --processes` also finds finished
runs. Inspect one with `lf graph show <RUN_ID>`; a finished run no longer appears
in `lf ps`.

## A Task stopped advancing

**Symptom:** the Task is not done, but nothing is moving, or its agent
exited.

```bash
lf task status <ID> --json
lf session usage --days 0 --task <ID> --json   # every Session input the Task ever made
lf session list --task <ID>
```

Dirty files under a live Graph are ongoing progress. An idle Task Graph does not
prove helper Sessions are idle, and a finished launcher does not prove its
interactive Session closed: check Sessions separately.

`lf task interrupt <ID>` stops a running Graph: an agent step ends at once, an
operation finishes and nothing follows, and the run exits 130. With nothing
live it records nothing and says so.

If the Task's worktree never finished being created, status keeps the Task and
names the exact path and branch to restore before launching.

## The Task is idle, but waiting on something

Task state says what the Task is doing (`idle`, `active`, `moving`, `shipping`,
…); whether anyone owns it is separate (`lf task claim`, `lf task unassign`). What it is waiting on (a child Task, review, CI, merge) is its
`reason`; Desktop's condition, with the evidence behind it, is in `lf task show`:

```bash
lf task status <ID> --json
lf task show <ID> --json
```

Resolve the named fact: open the conversation, inspect the child, repair CI,
merge. There is no run slot or PR-limit counter to clear.

## Rate limits

**Symptom:** runs fail with rate-limit or capacity errors.

Headless runs retry transient capacity, rate-limit, availability and transport
failures four times, continuing the same provider conversation so partial work
survives; the backoff tops out at 30 seconds.

A classified usage-limit or model-unavailable refusal moves a Graph step
through its configured model/harness order; other headless work stops with
the provider error. It never selects another login or
reads reset times. An explicit agent pins the harness; bare Claude can still
try its other models. When the whole order, or an explicit model pin, is
refused on a usage limit, the step waits 5, 15, 30, then 60 minutes, trying
again from the first entry after each; then it stops with the provider error
and a next step. A revoked login skips its provider's other models and, with no
other provider left, stops with how to sign in again. Other refusals stop at
once. Interactive Sessions retain the
refusal and require an explicit retry or move, even when attached to a Task.

```bash
lf set --global agents claude,codex
lf config settings --skill gate --explain
lf -a codex gate
```

Inspect the resolved order and any pin before retrying. Repair authentication
with the native client on the execution Machine, or wait for provider capacity.
For a stopped Task, `lf task run <ID>` continues the remaining steps. Earlier
steps stay done. [Native authentication](subscriptions.md) covers homes and resume.

## Agent not found

**Symptom:** `'claude' is not installed.`

The error names the install command. For Claude Code:

```bash
npm install -g @anthropic-ai/claude-code
claude
```

With no agent preference configured, lf uses the sole installed harness. If more
than one is installed, choose with `lf set --global default_agent claude` or pass
`-a claude` for one command. `lf config settings --explain` shows the selection.

## Context too large

**Symptom:** a run fails on context or token limits.

```bash
lf --context qa --docs src/small/    # see what the input is made of, launch nothing
lf session usage --context --task <ID>   # each step's input by source, against its budget
```

The provider loads `AGENTS.md` natively. lf adds `LOOPFLOW.md`, `scratch/` and
`.lf/wave/`, plus only what you pass with `--docs`: narrow the path or glob to
shrink it. Persistent docs belong in `docs:` in `.lf/config.yaml`.

## Disk filling up

```bash
scripts/disk-cleanup.sh            # reclaim now; each removal is logged to ~/.lf/logs/disk.log
scripts/disk-cleanup.sh install    # run the disk-cleanup skill hourly from crontab
lf machine prune                   # only lf's records: expired history, then vacuum
ls -la ~/.lf/loopflow.db ~/.lf/history.db   # state stays small
```

The script prunes every repository under `~/src`: finished worktrees, completed
Tasks' build output and merged branches through `lf wt prune`, and ignored
build output (`target`, `node_modules`, `.venv`, `.build`, `DerivedData`) that no process
holds and nothing wrote for 4 hours. It removes nothing under `~/.lf`: the
databases, its pre-migration clones (lf's own, see
`rust/loopflow-storage/src/MIGRATIONS.md`), captures, logs and account homes stay.
Anything a process holds open is never removed; when `lsof` fails, neither is
anything else that check protects.

Under cron, the `disk-cleanup` skill runs the script and reports what it freed,
what it kept and why, and worktrees still holding unpushed work; it deletes
nothing itself. Edit `scripts/disk-cleanup.sh` or
`.lf/skills/disk-cleanup.md` to change the policy. Provider transcripts follow
each provider's own retention; raise Claude's `cleanupPeriodDays` to keep more.

`history.db` grows with use. The Machine server keeps 90 days of Session
history, pruning every hour; turns are kept. `lf doctor` reports the file's
size and oldest rows, and warns when history is past retention. Deleting it
while nothing runs loses only past history and usage: Tasks, conversations,
Graph routing and `lf resume` read `loopflow.db`.

## Stale worktrees and branches

```bash
lf wt prune --dry-run              # every checkout, removed or kept, with the reason
lf wt prune                        # remove the finished ones and their branches
lf wt prune --all                  # remove every checkout no unfinished Task or process holds
lf wt prune --branches --dry-run   # finished branches on origin and locally, with reasons
```

A checkout goes when its PR merged or closed, its Task is done or abandoned,
or it has no Task or PR and HEAD has not moved for a week. Changes under
`scratch/` and `.lf/` never keep it; other uncommitted changes, an open PR and
recent activity do, until `--all`. Unfinished Tasks' checkouts, persistent
checkouts and any a running process holds are always kept. Unpushed commits
and source changes are saved as `refs/lf/pruned/<branch>/<commit>` first.

With `--branches`, a branch goes only with evidence at its exact head: a merged PR, a
finished Task's closed PR, a finished Task already in the default branch, or a
release candidate whose tag exists. An open PR, a worktree or an unfinished Task
always keeps it. Open PRs based on a deleted branch move to the default branch
first, so the PR above a landed one in a stack stays open. `lf wt delete <name> --force`
discards a checkout without saving anything.

A branch behind `main`:

```bash
lf sync                            # refresh main, keeping its unpublished commits, then merge it in
```

## Stray agent processes

```bash
lf machine doctor --json           # the strays check: each process, its pid and what it carries
```

A stray is an OS process carrying an lf Session or run (`LF_STEER_SESSION`,
`LF_PROCESS_ID`) that no live Session or Graph run claims: an agent left behind
when its terminal or holder was killed, or a child an agent started that
outlived it. Its children are listed under it. Doctor stops nothing; `kill <pid>`
one once you know what it is.

A run whose process died is not a stray. The Machine server checks each
recorded pid against the OS on its regular pass, marks the dead ones ended with
an unknown result and releases their Sessions, so `lf resume` and
`lf task run` work without any cleanup.

## Many `lf` processes running

```bash
ps -axo pid,etime,command | grep -E '[ /]lf( |$)'   # every lf process, its age and the binary that started it
lf session list --all --interactive all      # the conversations they serve
lf ps                                        # the Sessions and Graph runs lf knows are alive
```

A Machine runs one `lf server run`, one `lf holder run` per build with open
conversations, and one `lf` per running Graph or headless command. Everything
else belongs to a single conversation or view.

| In `ps` | What it is | Stopping it loses |
|---|---|---|
| `lf holder run` | Holds the terminals of every Task, Wave and repository conversation and Desktop shell opened on its build | Every agent and shell it holds; `lf resume` continues each conversation from history |
| `lf … task run`, `lf … graph run`, `lf … ship` | A Graph, landing watcher or ship running in a process of its own | The step in flight; `lf task status`, then `lf task run` what remains |
| `lf -b <skill>` | A headless run, a child of whatever started it | That run; its caller sees it fail |
| `lf session attach` | A terminal showing that conversation | Nothing; the conversation keeps running with no terminal showing it |
| `lf session serve <ID>` | A conversation an earlier build opened | The running agent and any turn in flight; `lf resume <ID>` ends it and continues from history |
| `lf session observe-status <ID>` | Records one conversation-status reading; it exits once written | Nothing; one that keeps running is from an older Desktop and stays until every terminal pane that Desktop opened has exited |
| `lf __codex-observe` | Records a Codex conversation's turns beside its client | Turn and usage records for that conversation; the conversation itself keeps going |
| `lf server run` | The Machine server | Desktop's connection until the server is started again |

Conversations remain running while their scope is open. Done Tasks end their
conversations during cleanup, about fifteen minutes after you stop viewing them,
once live turns, pending requests and Graphs settle. Abandoning a Task ends its
unviewed conversation. Typed input does not hold either cleanup.

A Desktop shell runs until you close its tab or its Task is cleaned up; hiding
it or quitting Desktop leaves it running.

A process started from a build other than the installed `lf`, such as
`target/debug/lf`, `/tmp/lf-*/bin/lf` or `Loopflow Dev.app`, is left over from
development. Tests end every process they start before they exit; one that
remains is a bug worth filing with its command line. Quitting a demo app leaves
its conversations running in the demo's holder. Use the demo's saved Machine environment to
`lf resume` each conversation and quit its agent before stopping anything by PID.

Stop processes one at a time by PID, after reading what each serves. Never stop
them by name or pattern (`pkill lf`, `killall lf`): that ends live
conversations in other checkouts along with the leftovers.

## A Wave isn't moving

Wave passes are finite; nothing runs between them unless you schedule one.

```bash
lf wave status <wave> --json
lf --wave <wave> wave-operate      # one pass, now
crontab -l                         # passes you scheduled yourself
```

A quiet Wave needs no service restart.

Next: the [`lf` reference](lf-reference.md) for every command and flag.
