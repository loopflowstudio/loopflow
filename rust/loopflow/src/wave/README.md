# wave — the listener and the resident

`lf wave <name>` starts a wave as **two processes**:

```
  lf wave <name>                      lf wave <name> --flowloop-only
  ┌───────────────────────┐  spawns   ┌──────────────────────────┐
  │ LISTENER (origin repo)│──────────▶│ RESIDENT (<repo>.<wave>) │
  │ pens · folds · doors  │           │ wave runner · queue │
  │ observer · supervisor │◀──deltas──│ GOAL.md seed · schedule  │
  └──────────┬────────────┘           └────────────▲─────────────┘
             └────────── /events?inbox=true ───────┘
```

- **The listener** is the channel made durable — hear / check / fold / tell,
  **vendor-free**. It holds every journal pen (the wave channel and the
  family's work lines), serves the doors, folds the store's worker
  observations, keeps the registry seat and the discovery pointer, and
  supervises the resident. It serves from the **origin repo** and creates no
  worktrees.
- **The resident** is the wave's flowloop (see `flowloop/wave.rs`): every
  turn is one `wave` flow (`wave_clarify → wave_pursue → wave_mutate`)
  run as a bounded headless child — no persistent vendor thread; continuity
  is GOAL.md + memory + the chat journal riding every pass's seed. It
  bootstraps and enters the wave's `<repo>.<wave>` sibling worktree — passes
  never run in the main checkout. Its input is its own wave's
  `/events?inbox=true` subscription (the same machinery as `lf sub`); its
  output is ordered turn deltas through the token-gated resident door. It
  never touches a journal file — the single writer stays with the listener.

One command runs both: the listener spawns the resident as a child `lf`
process (keeper spawns tenant) and both narrate into the same terminal.
`--no-flowloop` serves a dormant channel (`/health` reads `flowloop: null`);
`--flowloop-only` attaches a resident to an existing listener by hand — also
the respawn affordance, and one day the human-in-the-seat affordance.

- **One flowloop, two inputs.** Chat and progress share the same context. A
  message while idle starts a pass immediately. Messages during a pass queue
  (append-and-coalesce, never rejected) and one boundary pass drains them
  all — steering folds at pass boundaries by design, there is no mid-pass
  injection. The RESIDENT declares what it consumed in its `TurnOpened`
  delta's `answers`, and the listener validates against its pending fold
  before journaling `TurnStarted.answers`. Quiet for 4 hours with an empty
  queue → a heartbeat pass nudges the next orchestration skill, carrying the
  `<in_flight>` worker fold fetched from `GET /resident/context`.
- **Crons are the third deadline.** `crons: [{flow, schedule}]` in the
  wave's `GOAL.md` frontmatter (re-read live, no restart); a due schedule
  while idle opens a system pass — "cron due: <flow> — dispatch it" — and
  the flowloop dispatches with judgment. Mid-pass due dates fire at the
  boundary; occurrences older than 24h are missed, not replayed. The
  daemon's cron poller and `wave_crons` table died in the collapse.
- **The flowloop inhabits or delegates.** Each pass's seed is the rendered
  `GOAL.md` plus the coordinating-session discipline. It inhabits one
  context-sensitive hand with `lf loop <flow> "task" --wave <wave>` and
  delegates self-sufficient work with `--detach`; both use placed worktrees.
  The LISTENER polls the shared store
  and journals `RunObserved`/`RunCompleted` observations — every ~10s and
  once per `GET /resident/context` (the resident calls it before each pass).
- **Interrupts kill the pass child.** The resident sends an `Interrupting`
  state delta, kills the child, and closes the turn
  (`TurnFinished{interrupted}`); non-empty interrupt text queues for the
  next pass. The listener keeps its own 20s janitor for a resident gone
  fully silent: past it the open turn force-finalizes server-side and late
  wire deltas for it are dropped until the next `TurnOpened` — the
  anti-wedge lives where the pen is.
- **Failure is a dead process.** Three consecutive failed passes (nonzero
  exit, spawn failure, timeout) → the resident reports `FlowloopState::Failed`
  over the wire and exits nonzero. The listener's supervisor owns revival:
  the process-level respawn ladder (5m/15m/45m, last rung repeating; reset
  by a completed turn), and a human message respawns immediately — talking
  to the wave brings it back. Resident death is detected by process exit
  (spawned) or a pid probe (attached); an open turn left behind is
  force-closed `failed`, so the journal never dangles. The listener dying
  ends the resident too: its subscription drops and it exits cleanly — the
  keeper is gone; tmux/systemd restarts are the human's arrangement.
- **One brain per wave.** On boot the listener writes itself a `WaveAgent`
  session row in the shared store (source `wave_server`, endpoint + pid in
  `env`; the db IS the registry). The row is marked terminal on shutdown or
  any termination signal; a crashed listener's row is closed by the next
  boot's pid probe. A second `lf wave` refuses to start naming the live
  session unless `--force` takes over. A wave the store has never seen gets
  its row created at boot. No registry store on the machine → warn once, run
  fully functional, with one file-level floor: a `.wave-endpoint` that
  answers `GET /health` for this wave also refuses a second server.

Truth is the per-wave append-only journal —
`.lf/journal/waves/<name>/journal.jsonl` under the **origin (main) repo**,
per-machine, never committed. The in-process state (`WaveRuntime`) is a fold
of it: the `thread` and the flowloop state are rebuilt from the journal on
boot, so a restart keeps the full conversation and turn ids continue
monotonically. The journal event vocabulary predates the pass model —
`TurnStarted`/`TurnItem`/`TurnSteered`/`TurnFinished`/`FlowloopState` are
all journaled by the listener on receipt from the wire (`TurnSteered` is
vestigial: pass-based residents never emit it). `wave/<name>/MEMORY.md`
seeds the flowloop (read from the origin repo), and the live listener holds
its pen: `lf memory update` POSTs the compiled checkpoint. `lf memory add`
publishes a replayable fact to the stream without accreting raw bullets into
the checkpoint. `lf memory log` prints those add-stream facts since the last
checkpoint.

## The resident wire

Two directions (full DTO discipline, fixture at
`tests/fixtures/dto/resident_deltas.json` — Rust↔Rust only; Swift/Python
don't consume this wire):

- **Resident → listener**: `POST /resident/deltas {deltas: [...]}` — the old
  in-process TurnSink vocabulary promoted to the wire: `turn_opened
  {answers}`, `turn_text`, `turn_item`, `turn_usage`, `turn_finished {status,
  cost_usd}`, `turn_steered {answers}`, `flowloop_state {to, reason}`
  (interrupting/failed — turning/idle are derived from the turn deltas).
  Sent serially, so per-turn order is the transport's order; turn ids never
  ride the wire — the listener mints them from its journal seq. Plus
  `POST /resident/attach {pid}` (liveness registration + revival; returns
  `{wave}`) and `GET /resident/context` (`{in_flight}`; serving it freshens
  the store fold).
- **Listener → resident**: `GET /events?inbox=true` adds `inbox` SSE frames
  to the primary subscription — `{id, op, text, from}` per resident-directed
  op, the pending queue replayed on connect, bare interrupts live-only with
  `id: null`. The default `/events` stream is byte-identical to the
  pre-resident wire.
- **Auth (stopgap)**: the resident door requires this boot's token in the
  `x-lf-resident-token` header. The listener generates it at bind, passes it
  to a spawned resident via `LF_WAVE_RESIDENT_TOKEN`, and publishes it at
  `wave/<name>/.wave-resident-token` (owner-only) for attached residents —
  the same filesystem-trust domain as the endpoint pointer. When a human or
  a remote flowloop can hold the seat, the token becomes a gatekeeper-issued
  credential.

## Channels

A CHANNEL is a named stream — journal + thread + subscribability. The wave's
own channel is its name (`goals`), journal under the origin repo. A work
line's channel is the ownership name — exactly the worktree basename minus
the repo prefix (`goals.148e0e02`) — journal IN that worktree
(`.lf/journal/waves/<channel>/journal.jsonl`): it travels with the branch
and dies with it (channels are conversations, not records). The listener
serves the whole FAMILY: it holds the pen for every child journal (all pens
in one process), folds each channel separately, and the family view folds
upward. Child channels have no flowloop and no memory — pure streams. Names
are topics, dots are the tree; subscription is by name or prefix. Placed `lf`
runs mint the work line's channel with its worktree — journal initialized
there, `LFD_CHANNEL` in the worker's env, and one `POST /channels` knock so
the wave's thread shows "work line <name> opened".

The speech surface is `lf chat` — the same verb for flowloops, workers,
humans, and scripts (the one-door exec convention). It resolves its target
CHANNEL from context (`LFD_CHANNEL`, else `LFD_WAVE_ID`, else the worktree
name — inside a work-line worktree that is the work line's own channel: speak
locally; `--parent` walks `parent_wave_id` through the registry; `--wave
<name>` is explicit, dotted names addressing a work line through its family
head), finds the FAMILY HEAD's live endpoint via its WaveAgent session row
(falling back to `.wave-endpoint`), and POSTs a `say` op with the channel
field. Publish-to-no-subscriber drops: with no wave context anywhere,
`lf chat` and `lf memory` writes exit 0 with one stderr note. A resolvable
wave whose server is down errors instead (mail to a dead wave bounces, it
doesn't vanish). Dispatched workers finish with an `lf chat` report — it
arrives in the thread with a `from` byline and wakes the flowloop like any
input.

`lf sub [NAME] [--json]` is the read half: follow the family's `/events`
stream (turns, flowloop state, memory curation, memory adds) until killed,
reconnecting with backoff and re-resolving the endpoint across server
restarts. `lf sub goals` follows the whole family; `lf sub goals.148e0e02`
follows one work line. The resident is the subscription's second customer —
same machinery, plus the inbox scope.

The context flows back out the same way: every `lf` run born inside a wave
inherits ambient context by CHANNEL in its assembled prompt —
`<lf:wave-chat-recent>` and `<lf:wave-memory>` (see
`engine/wave_context.rs`).

## Wire contract (snake_case, stable)

The listener binds a loopback port. Loopflow finds it via the discovery
pointer, under the origin repo's `wave/<name>/`:

```
wave/<name>/.wave-endpoint         →  127.0.0.1:<port>   (address only; removed on shutdown)
wave/<name>/.wave-resident-token   →  this boot's resident token (owner-only)
```

| Method + path             | Behavior |
|---------------------------|----------|
| `GET /health`             | `{status, flowloop, wave, turns, workers, uptime_seconds}`; `status` is channel liveness — always `serving` while the process answers; `flowloop` is the flowloop's state (`idle \| turning \| interrupting \| failed`), or null while no resident was ever spawned or attached (`--no-flowloop` serves dormant) — a live channel whose resident died reads `serving` + `failed`; `workers` counts observed in-flight worker runs |
| `GET /conversation`       | `{turns: [Turn]}` — the whole thread; `?limit=N` tails the last N turns (open turn included) |
| `GET /events`             | SSE, the family's one unified stream. Scope: `?channel=<name>` (one channel), `?prefix=<name>` (subtree), default = whole family; names outside the family 404. Event names: `state` (flowloop-state name, on subscribe + every transition; primary only), `turn` (a `Turn` JSON; replay then live; child-channel turns carry an extra `"channel"` key; ids repeat — each frame replaces the client's previous state for that (channel, id)), `memory-add` (full added facts since the last externalization, replay then live; primary only), `memory` (curation summaries, live-only; primary only), and — only with `?inbox=true`, the resident's subscription — `inbox` (`{id, op, text, from}`; pending replay + live ops; bare interrupts ride `id: null`). |
| `POST /messages {op, text, from?, channel?}` | `op` required: `message` (human speech: queued, the next pass answers it), `steer` (degrades to `message` — steering folds at pass boundaries), `interrupt` (kill the open pass; non-empty text queues for the next pass), or `say` (an attributed emission — `lf chat`; `from {session_id?, label}` required for `say`, rejected otherwise). `channel` null = the wave channel; a child name lands in that work line's journal (404 outside the family). Returns `{turn, state}`. |
| `POST /channels {name, run_id}` | The dispatch knock: journals `ChannelOpened` on the wave channel. Idempotent on `run_id`; 404 outside the family. Returns `{turn}`. |
| `GET /memory`             | `{content}` — the wave's MEMORY.md (origin repo). |
| `GET /memory/log`         | `{facts}` — add-stream facts since the last curation, oldest first. |
| `POST /memory {op, content, summary}` | `op`: `update` replaces `MEMORY.md`; `add` publishes one replayable fact. `summary` null → first non-empty content line. Returns `{summary}`. |
| `POST /resident/attach {pid}` | Resident door (token-gated): register the resident's pid, revive a failed flowloop. Returns `{wave}`. |
| `POST /resident/deltas {deltas}` | Resident door (token-gated): ordered turn deltas → the journal fold. Returns `{accepted}`. |
| `GET /resident/context`   | Resident door (token-gated): `{in_flight}`; freshens the store observations. |

### Turn

```json
{
  "id": "turn-3",
  "role": "user | assistant",
  "text": "…",
  "status": "pending | running | completed | failed | interrupted",
  "items": [ ConversationItem, … ],
  "created_at": "2026-07-04T00:42:03.412861Z",
  "from": "worker"
}
```

`items` are the tool/command/file/message artifacts a pass produced, in
order (`ConversationItem` — see `chat/types.rs`). User turns
carry empty `items`. Turn `id`s are a single monotonic `turn-<n>` sequence
across all sources. `from` is the speaker byline of an attributed emission
(`lf chat`); null for the flowloop's own turns and plain user turns.

## Demo

```
lf wave demo
# → lf wave · demo · listener on http://127.0.0.1:52306 · spawning resident (Ctrl-C to stop, …)
# → lf wave · demo · resident flowloop · listener http://127.0.0.1:52306 · worktree …/demo-repo.demo

curl 127.0.0.1:52306/health
curl -X POST 127.0.0.1:52306/messages -H 'content-type: application/json' \
     -d '{"op":"message","text":"status?"}'
curl 127.0.0.1:52306/conversation
curl -N 127.0.0.1:52306/events
```

The full guided walk — chat, steer, interrupt, worker dispatch, attributed
reports, restart, teardown — is `scripts/demo_wave.sh` (`--smoke` for a
zero-model-turn sanity pass). The two-process topology has its own ignored
live test, `cargo test -p loopflow --test wave_live_smoke -- --ignored`.
