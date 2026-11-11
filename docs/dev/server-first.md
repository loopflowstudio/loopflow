---
layout: default
title: Server-first Sessions
---

# Server-first Sessions

Planned in LOO-819. Reading a live turn from the transcript (LOO-822) is
built; writing those turns to the store is not, nor is the rest.
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

Bare and raw conversations have neither signal. For them "waiting on you" is
what the transcript shows: a question with no answer yet, or a turn that
ended. Not built: such a turn reads as running today.

Codex history reads from its rollout files; no app-server call is needed for
turn state or usage. Rollouts under `~/.codex/sessions` read on November 10,
2026 carry `event_msg` records of type `task_started`, `task_complete` (with
`turn_id`, `started_at`, `completed_at`), `turn_aborted` and `token_count`
(`info.last_token_usage`, `info.total_token_usage`). They carry no approval
or input requests, so "waiting on you" stays with the status reports above.
A Claude Code 2.1.322 transcript read the same day ends a turn with an
`assistant` entry whose `stop_reason` is `end_turn`, usually followed by a
`system` entry of subtype `turn_duration` (33 of 49 turns in one transcript),
so `end_turn` with nothing after it is the signal and `turn_duration` a
confirmation.

One reader per harness (`transcript/claude.rs`, `transcript/codex.rs`) turns a
transcript's rows into the turns `lf history show` prints and the runs those
turns belong to: one run from what woke the agent until it hands back, with
its model, usage, and whether it stopped on a question. On demand,
`lf session list` and `lf session card` give that reader the transcript's tail
for any conversation with no hook report (`transcript/live.rs`). Watched, the
server keeps one filesystem watch per provider home (`transcript/watch.rs`);
an event makes the shared observation cache read that transcript within a
second, a thirty-second reconcile catches a missed one, and every
subscription reading it shares the one reading. Raw conversations are
`claude` or `codex` processes with no `lf` or provider ancestor, paired with
the newest transcript written in their directory since they started
(`transcript/raw.rs`). Claude's kernel process name is its version, so its
executable path (`…/claude/versions/<version>`) identifies it. User docs:
[Sessions](../sessions.md#see-what-each-one-is-doing).

Provider transcripts expire, and the cost-per-PR work (LOO-732) reads per-turn
`usage` and `completed` rows from the store, so a transcript read also writes
them (`transcript/facts.rs`): when a provider lf launched exits, each finished
run the store holds no completion for gets both rows, dated when the run
ended, in the live stream's payload shape plus `"source":"transcript"`. A run
is already recorded when a completion carries its id (Claude's request row,
Codex's turn, the same ids the live stream uses) or was observed while it
ran, since the live recording folds queued input into one turn. These rows
carry no `cost_usd`. Not written: a run cut off with no end in the transcript,
and anything for a raw conversation, which has no Session to hold rows. What
each transcript carried on November 10, 2026 (Claude Code 2.1.322, Codex
0.167.0):

| Fact | Claude | Codex |
| --- | --- | --- |
| Tokens | each `assistant` message's `usage` | `token_count`, last and total |
| Model | `message.model` | `turn_context.model` |
| Turn wall time | `turn_duration.durationMs`, not on every turn | `task_complete.duration_ms`, `turn_aborted.duration_ms` |
| Stop reason | `message.stop_reason`, `system` rows of subtype `api_error` | `turn_aborted` |
| Provider version | `version` on every row | `session_meta.cli_version` |
| Cost, model time | none per turn; totals in `cost-state` | none |

Claude's `cost-state` row holds `totalCostUSD`, `totalAPIDuration` and
per-model usage and cost. It was the last row of 24 of the 60 newest
transcripts and absent from 35; a resumed one held six. What it totals and
when Claude writes it are not established. Per-turn `cost_usd` and
`duration_api_ms` come only from the live `result`.

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
- Where a transcript-read Claude turn gets its cost: the `cost-state` total
  on a run's last turn, or none.
- Whether a raw conversation's turns reach the store, which holds turns only
  for a Session.
- Whether the server becomes the only process that opens the store. Hooks and
  views already stop opening it under this plan; Graph runners and one-shot
  commands still do.
- How the holder is started on macOS and Linux so the service manager leaves
  it alone when the server's service is replaced.
