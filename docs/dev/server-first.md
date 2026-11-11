---
layout: default
title: Server-first Sessions
---

# Server-first Sessions

Planned in LOO-819. Reading a live turn from the transcript and writing a
finished conversation's turns to the store at exit (LOO-822) are built; the
rest is not.
[Client API server](server.md)
and the [transport README](../../rust/loopflow/src/transport/README.md)
describe what runs today. Each piece below lands as its own Task and updates
those documents when it does. Delete this page when the last piece lands.

How Paseo, T3 Code, Paradev and Rex handle the same questions, and the traps
they hit, is in [Server protocols elsewhere](research/server-protocols.md).

## Why

Every conversation has its own detached owner process holding the agent on a
private PTY, and most have one or two more beside it: a status pipe, a Codex
observer, a Task-update watcher. One Machine measured on November 10, 2026 had
281 `lf` processes using 11.6 GB, 256 of them per-conversation residents, and
73 of its 95 owners were more than a day old. Nothing visible holds those
owners, so nothing shows how many conversations are alive.

## Two kinds of Session

```text
bare                                  managed
lf implement                          lf resume --task LOO-1, Desktop
    |                                     |
    v                                     v
agent, a child in your terminal       view --+
                                      view --+--> Machine server --> holder --> agent on a PTY
                                      view --+        |
                                                      +-- store, hooks, Task updates, steers
```

| | Bare | Managed |
| --- | --- | --- |
| Started by | `lf <skill>`, `lf graph …`, `lf -i` in a terminal | Task and Wave commands, Desktop |
| Holds the process | Your terminal | A holder the Machine server controls |
| Resident `lf` processes | None beyond the launching parent | The server and its holders, shared by every Session |
| Hooks | None | Steers and Task updates in; a wake-up to the server out |
| Views | The terminal it runs in | Any number, in Desktop or any terminal, all live |
| Outlives its terminal | No | Yes |

A Session is the same record in both. What differs is who holds its current
process.

### Bare

`lf` compiles the prompt and context, starts the agent as a foreground child
of the calling terminal, waits, and reads the provider's transcript when the
child exits. It installs no hooks and needs no server. A host that shows the
foreground program, such as Rex, shows the agent.

Persistence, reattachment and status display belong to the terminal. Opening a
bare Session somewhere else starts a new process in the same Session from the
provider's history, exactly as for a conversation started by plain `claude` or
`codex` ([Bring in one you started elsewhere](../sessions.md)). A process
still open elsewhere is not one `lf` stops.

### Managed

The Machine server starts the agent on a PTY kept by a holder. Every place the
Session is opened is a view of that one process: typing in any view shows in
all of them. In a terminal the view is a small `lf` relay, so a host such as
Rex shows `lf` there, not the agent.

- **Size.** The most recently opened view owns the terminal size; other views
  render that grid. When the owner closes, ownership falls back to the next
  most recently opened view. A reconnect is not an open, and a Desktop pane
  counts as open only while it is shown.
- **Input.** Every view can type. There is no controlling view, takeover
  token or Move here.
- **Joining late.** The server keeps the terminal emulator and paints a
  joining view from it. The holder keeps only recent raw bytes. After a server
  restart the emulator is rebuilt from those bytes, or the agent is asked to
  repaint by a size nudge; scrollback from before the restart may be absent.
- **Attach never creates.** A view of a Session whose process is gone says so
  and offers resume.

## Processes

| Process | Restarts | Holds |
| --- | --- | --- |
| Machine server | On every install | The store connection, the work model, hook and client connections, terminal emulators, observation |
| Holder | Never for an upgrade | PTYs, their child agents, a ring of recent output per PTY |
| Graph runner | With its Graph | One running Graph and its headless step children |

An install never ends a conversation. A holder keeps the build it started with
until its Sessions end; new Sessions go to a holder on the current build, so
several holders may be alive at once. The server speaks to older holders, which
keeps the holder's protocol small: start, write, resize, read from a byte
position, report exit.

A holder is started so that stopping or replacing the server's service does not
stop it, and it claims its socket with a lock the kernel releases on death. It
never unlinks a socket path before binding. A dropped connection to a holder
says nothing about its agents.

## Telemetry

The provider's transcript is the record of turns, tools and usage for every
conversation: managed, bare and raw. Two signals sit beside it for managed
Sessions:

- The agent's own status reports in its output show whether the Session is
  working or available for a reply, including a permission or a question.
  The server's emulator reads them; no per-pane status process is needed.
- A hook at a turn boundary tells the server to read the transcript now.

Hooks reach the server at its fixed socket path with a short deadline. A hook
that cannot reach it logs a gap and lets the turn continue.

Bare and raw conversations have neither signal. They read as available from
what the transcript shows: a turn that ended. A question with no answer yet
is not in it: Claude Code 2.1.322, run on November 10, 2026, wrote an
`AskUserQuestion` call to its transcript only with its answer, 55 seconds
after the call, and an Esc before any reply wrote nothing, leaving the
prompt's `user` row last until the next message.

Codex history reads from its rollout files; no app-server call is needed for
turn state or usage. Rollouts under `~/.codex/sessions` read on November 10,
2026 carry `event_msg` records of type `task_started`, `task_complete` (with
`turn_id`, `started_at`, `completed_at`), `turn_aborted` and `token_count`
(`info.last_token_usage`, `info.total_token_usage`). They carry no approval
or input requests, so availability there stays with the status reports above.
A Claude Code 2.1.322 transcript read the same day ends a turn with an
`assistant` entry whose `stop_reason` is `end_turn`, usually followed by a
`system` entry of subtype `turn_duration` (33 of 49 turns in one transcript),
so `end_turn` with nothing after it is the signal and `turn_duration` a
confirmation.

One reader per harness (`transcript/{claude,codex,pi,opencode}.rs`) turns a
transcript's rows into `Turn`s, for `lf history show` and the live reading
alike: one turn from what woke the agent until it hands back, with its id,
state, model and usage, and the requests and agent work inside it as items.
Pi ends a turn at an assistant message whose `stopReason` is not `toolUse`;
OpenCode at a completed assistant message whose `finish` is not
`tool-calls`, or its `idle` row. OpenCode keeps every conversation in one
SQLite store, read-only here: its `session_message` rows are the messages
`opencode session export` writes, and a query on them took about 4 ms on
November 11, 2026 where the export took 490 ms, so history and the live
reading both query the store and no reader runs a subprocess. On demand, `lf session list` and `lf session card`
give that reader the transcript's tail for every live conversation
(`transcript/live.rs`): the conversation's turn is the transcript's last one.
When the transcript leaves that turn open and lf recorded a completion under
the same id, the recorded end closes it. Ids are the only match. Known and
left wrong: a prompt lf's live recording folded into an earlier turn, then
stopped with Esc before any reply, reads as running until the next message.
A Session is available from its agent's status reports when it has them, else
when its turn ended or was interrupted. Watched, the
server keeps one filesystem watch per provider home (`transcript/watch.rs`),
as does a CLI `lf session list --subscribe`, which reads in its own process;
an event makes the shared observation cache read that transcript within a
second, a thirty-second reconcile catches a missed one, and every
subscription reading it shares the one reading. Pi's root is
`~/.pi/agent/sessions`; OpenCode's whole home is watched, and any write to
its store or its write-ahead log hints every OpenCode reading. Raw
conversations are `claude`, `codex`, `pi` or `opencode` processes with no
`lf` or provider ancestor (`transcript/raw.rs`); Pi runs as `node` with Pi's
CLI as its script. A Claude process is paired with the conversation its
`sessions/<pid>.json` in Claude's home names; Claude rewrites that file on
`/clear` and `/resume`. A process without one takes the newest transcript
written in its directory since it started that no other live Claude process
names. Codex, Pi and OpenCode have no such file, so the newest rule pairs
them: Pi's newest session file under its directory's
`sessions/--<path>--/`, OpenCode's newest `session_v2` row for the
directory. One whose conversation an ended Session recorded lists
under that Session's title. Claude's kernel process name is its version, so its
executable path (`…/claude/versions/<version>`) identifies it. User docs:
[Sessions](../sessions.md#see-what-each-one-is-doing).

A bare run in a Task's checkout counts toward that Task, and the cost-per-PR
work (LOO-732) reads per-turn `usage` and `completed` rows from the store.
The transcript is the only writer of `usage` rows (`transcript/facts.rs`):
one row per turn id, holding that turn's tokens and model, not a running
total. It is written at each Stop moment (Claude's and Codex's Stop hook, Pi's
`agent_settled`, OpenCode's `session.execution.succeeded`), for each ended
turn at the transcript's end, and again when the provider exits, for every
turn; a turn that has a row keeps it. When a hook fires before the
transcript shows the turn's end (not measured for either provider), that
turn's row lands at the next turn's end or at exit. The live stream (`harness/fact_writer.rs`) writes
`started` and `completed` and no `usage`. Pi's stream names a turn by lf's
own request id, which Pi's session file never holds, so a Pi turn's `usage`
row (Pi's entry id) and its stream `completed` row carry different ids; the
exit read matches them by time. At exit, each turn the store holds
no completion for also gets a `completed` row, dated when the turn ended. A
last turn the transcript leaves open was cut off, since its provider is gone:
it is written `interrupted`, ended at the transcript's last row. A turn's
completion is already recorded when one carries its id (Claude's request row,
Codex's turn, the same ids the live stream uses) or was observed while it
ran, since the live recording folds queued input into one turn; that second
test is by time and is the one place times still match turns. Claude and
Codex usage rows carry no `cost_usd`: tokens come first, input, cache read,
cache write and output kept apart, and dollar accounting is later work. Pi
and OpenCode report each model call's cost, so their rows carry the turn's
sum as `cost_usd`. The live `result`'s
`total_cost_usd` is kept on Claude's stream-written `completed` row as
`cost_usd`, the process's running total. Not written: anything for a raw
conversation, which is listed but not tracked. What each transcript carried
on November 10 and 11, 2026 (Claude Code 2.1.322, Codex 0.167.0, Pi's
session format 3, OpenCode 2.2.1):

| Fact | Claude | Codex | Pi | OpenCode |
| --- | --- | --- | --- | --- |
| Tokens | each `assistant` message's `usage` | `token_count`, last and total | each `assistant` message's `usage` | each `assistant` message's `tokens` |
| Model | `message.model` | `turn_context.model` | `message.model` | `model.id` |
| Turn wall time | `turn_duration.durationMs`, not on every turn | `task_complete.duration_ms`, `turn_aborted.duration_ms` | none; `durationMs` per model call | none; `time.created` and `time.completed` per message |
| Stop reason | `message.stop_reason`, `system` rows of subtype `api_error` | `turn_aborted` | `stopReason`, `errorMessage` | `finish`, `error`, `idle.outcome` |
| Provider version | `version` on every row | `session_meta.cli_version` | `session.version` (format only) | `session_v2.version` |
| Cost, model time | none per turn; totals in `cost-state` | none | `usage.cost.total` per call | `cost` per message |

Claude's `cost-state` row holds `totalCostUSD`, `totalAPIDuration` and
per-model usage and cost. It was the last row of 24 of the 60 newest
transcripts and absent from 35; a resumed one held six. What it totals and
when Claude writes it are not established. Cost and `duration_api_ms` come
only from the live `result`; of the two, lf keeps the cost.

## Pieces

Each is one Task and one PR. The first four do not depend on each other.
Desktop already sends pane status as `session.status` writes on its server
connection, with no process per pane. That operation is temporary: it is
deleted when the server's emulator reads status itself.

| Piece | Removes | Needs |
| --- | --- | --- |
| Bare launches go direct: no owner, agent in the terminal's foreground | Owners and relays for bare runs | |
| Hooks call the server, not the store | Store opens inside turn boundaries | |
| Transcript telemetry, on demand and watched by the server | The telemetry gap bare launches open | |
| Retire leaked processes and find the tests that leak them | Stale owners, test and demo leftovers | |
| Holder: PTYs move from per-Session owners to a holder beside the server | Per-Session owners | The managed rule below |
| Every view types; size follows the newest view | Takeover tokens, Move here | Holder |
| Observers become server registrations | `__codex-observe`, Task-update watchers | Hooks call the server |
| Bare launches drop their hooks | Hook installs for bare runs | Transcript telemetry |
| CLI cleanup: `session serve`, `session attach`, the detach key, `session.end_detached_after` | The owner's surface | Bare direct, holder |

Server operations for these pieces are added under one protocol revision
bump; pieces that add operations take the next revision together rather than
one each.

## The managed rule

A launch is managed when it comes through a Task or Wave command or from
Desktop. A bare `lf` typed inside a Task's checkout stays bare and records the
Task as its ancestry. One function answers this for every launch path.

Any `lf` run in a Task's checkout counts toward that Task: its usage and time
roll up into the Task's cost. Design the bare path on the assumption that most
bare runs are subagents started by a Task's agent, not a person at a prompt:
nobody is watching, the caller waits on the exit and reads the output, and the
run ends with its parent.

## Open

- Whether Claude and Codex repaint fully on a size nudge.
- When a run's checkout and its calling Session belong to different Tasks,
  which one its cost counts toward.
- Whether a bare run outside every Task and Wave checkout gets usage rows.
  The exit read writes them for any provider lf launched.
- How long the store keeps turn rows once the provider's transcript is the
  record: as long as today, expired, or sent elsewhere.
- Whether the server becomes the only process that opens the store. Hooks and
  views already stop opening it under this plan; Graph runners and one-shot
  commands still do.
- How the holder is started on macOS and Linux so the service manager leaves
  it alone when the server's service is replaced.
