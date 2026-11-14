---
layout: default
title: Server-first Sessions
---

# Server-first Sessions

Planned in LOO-819. Held terminals, headless work as plain processes, hooks
calling the Machine server
([`session.boundary`](../../rust/loopflow/src/server/README.md#turn-and-tool-boundaries)),
the server delivering Task updates and recording Codex turns itself
([Waking an idle conversation](../../rust/loopflow/src/server/README.md#waking-an-idle-conversation)),
Desktop's `session.status` and transcript telemetry (LOO-822) are built.
[What runs on a Machine](index.md#what-runs-on-a-machine) is the model, and
the [transport README](../../rust/loopflow/src/transport/README.md) describes
the held terminals and headless launches behind it. This page keeps the
telemetry design, the pieces not yet built and the open questions. Delete it
when the last piece lands.

How Paseo, T3 Code, Paradev and Rex handle the same questions, and the traps
they hit, is in [Server protocols elsewhere](research/server-protocols.md).

## Why

Every conversation had its own detached owner process holding the agent on a
private PTY, and most had one or two more beside it: a status pipe, a Codex
observer, a Task-update watcher. One Machine measured on November 10, 2026 had
281 `lf` processes using 11.6 GB, 256 of them per-conversation residents, and
73 of its 95 owners were more than a day old. Nothing visible held those
owners, so nothing showed how many conversations were alive.

## Telemetry

The provider's transcript is the record of turns, tools and usage for every
conversation: managed, bare and raw. Two signals sit beside it for managed
Sessions:

- The agent's own status reports in its output show whether the Session is
  working or available for a reply, including a permission or a question.
  The server's emulator reads them; no per-pane status process is needed.
- A hook at a turn boundary sends the server the provider's callback, and
  the server records the turn from it. The hook then reads the Session's
  steers from the store itself.

Hooks reach the server at its fixed socket path with a short deadline. A hook
that cannot reach it logs a gap and lets the turn continue. A managed launch
starts the server before its agent and fails when none comes up. A bare run,
headless or in a terminal, has its two context hooks and the steer check every
launch has.

Bare runs at a terminal or headless, and raw conversations, have neither
signal. They read as available from
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
`~/.pi/agent/sessions`; OpenCode's whole home is watched, and any event in
it hints every OpenCode reading: FSEvents reports no write to OpenCode's store
while OpenCode holds it open, but OpenCode writes worktree snapshots under
`snapshot/` as each step starts and finishes (seen 2026-11-11, OpenCode
2.2.1). Raw
conversations are `claude`, `codex`, `pi` or `opencode` processes with no
`lf` or provider ancestor (`transcript/raw.rs`); Pi runs as `node`, its
arguments its CLI script until it retitles itself `pi`. A Claude process is
paired with the conversation its `sessions/<pid>.json` in Claude's home
names; Claude rewrites that file on `/clear` and `/resume`. A process without
one takes the newest transcript written in its directory since it started
that no other live Claude process names. Codex, Pi and OpenCode have no such
file, so the newest rule pairs them: Pi's newest session file under its
directory's `sessions/--<path>--/`, OpenCode's newest `session_v2` row for the
directory. The rule skips a conversation a Session wrote last: one whose
Session runs, or was last written before its Session ended (or stopped with
no end recorded). It cannot see a raw process that exited: its conversation
stays newest, so a second raw process idle in the same directory lists it
until that process writes again. One written after the Session that
recorded it ended lists under that Session's title. Claude's kernel process
name is its version, so its executable path (`…/claude/versions/<version>`) identifies it. User docs:
[Sessions](../sessions.md#see-what-each-one-is-doing).

A bare run in a Task's checkout counts toward that Task, and the cost-per-PR
work (LOO-732) reads per-turn `usage` and `completed` rows from the store.
The transcript is the only writer of `usage` rows (`transcript/facts.rs`):
one row per turn id, holding that turn's tokens and model, not a running
total. Turn-end hooks send `session.boundary` to the Machine server and open
the store only to claim their own steers. Before answering a Stop, `server/boundary.rs::answer`
reads the transcript's tail and records each complete ended turn (Claude's
and Codex's Stop hook, Pi's `agent_settled`, OpenCode's
`session.execution.succeeded`). Bare runs have no turn-end hook.
The launching process reads every turn at exit, through
`harness/native_exit.rs` or `process/runtime.rs`; that write needs no server.
A turn that has a row keeps it. If a hook arrives before the transcript
shows the end, the next turn-end read or the exit read can record it. The
ordering has not been measured; recording failures are logged and do not
fail the turn or exit. The live stream (`harness/fact_writer.rs`) writes
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
`total_cost_usd`, the process's running total, is kept under that name on
Claude's stream-written `completed` row, so no reader sums it as one turn's
cost. A turn has one `usage` row; a second, different receipt for it is a
conflict. Not written: anything for a raw
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

Each is one Task and one PR. Desktop already sends pane status as
`session.status` writes on its server connection, with no process per pane.
That operation is temporary: it is deleted when the server's emulator reads
status itself.

| Piece | Removes | Needs |
| --- | --- | --- |
| Every view types; size follows the newest view; the server's emulator answers every terminal query and replies coming from a view's terminal are dropped before the PTY | Takeover tokens, **Move here**, and Desktop's relay status handshake (`LF_SERVER_STATUS`, "Opening… Input paused") | |
| Retire leaked processes and find the tests that leak them | Test and demo leftovers | |

Server operations for these pieces are added under one protocol revision
bump; pieces that add operations take the next revision together rather than
one each.

### Every view types

What a held terminal's views would become:

- **Size.** The most recently opened view owns the terminal size; other views
  render that grid. When it closes, ownership falls back to the next most
  recently opened view. A reconnect is not an open, and a Desktop pane counts
  as open only while it is shown.
- **Input.** Every view can type. There is no controlling view, takeover
  token or **Move here**.

## Open

- When a run's checkout and its calling Session belong to different Tasks,
  which one its cost counts toward.
- Whether a bare run outside every Task and Wave checkout gets usage rows.
  The exit read writes them for any provider lf launched.
- How long the store keeps turn rows once the provider's transcript is the
  record: as long as today, expired, or sent elsewhere.
- Whether hook-recorded turns stay a source of their own beside the
  transcript, or the hook becomes a wake-up that tells the server to read the
  transcript. `session.boundary` carries the whole callback, so it serves
  either.
- Whether the server becomes the only process that opens the store. Views
  already do not open it; Graph processes, one-shot commands and every hook
  claiming its own steers still do.
- Whether Claude or Codex re-send terminal modes (bracketed paste, focus,
  Kitty keyboard) on resize, which decides what a crashed server's rebuilt
  emulator gets wrong.
- How the server holds its store. Each `session.boundary` opens a connection,
  and each open validates the whole migration history: 5 ms alone, and about
  85% of a boundary's time when 50 Sessions take boundaries at once (p50
  550 ms of 640 ms on a loaded Machine). Validating once per schema change, or
  keeping connections, removes it.
- Protocol negotiation between an `lf` and a server on another build. Today
  a server that speaks the same revision but lacks an operation welcomes the
  hook and answers `invalid {kind}`. Negotiation reuses the `hello`/`welcome`
  exchange (`revision`, `min_revision`) Desktop already sends; no second
  handshake. Until then, upgrade the server.

## Answered

- **Size nudge.** Measured November 10, 2026 (Claude Code 2.1.322,
  codex-cli 0.167.0, idle and after a 30-line answer): resizing one column
  or row and back repainted every non-blank row of the visible screen and the
  cursor from about 1.5–4 KB of output. Scrollback above the screen is not
  repainted.
- **Holder start.** A `setsid` child of a launchd agent survived
  `launchctl bootout` with and without `AbandonProcessGroup`; the server's
  plist sets it and its systemd unit uses `KillMode=process`. The server
  spawns the holder with `setsid`; no separate service.
