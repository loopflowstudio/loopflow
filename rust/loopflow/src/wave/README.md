# wave — the listener and the resident

`lf serve <name>` starts a wave as **two processes**:

```
  lf serve <name>                     lf __resident <name>
  ┌───────────────────────┐  spawns   ┌──────────────────────────┐
  │ LISTENER (origin repo)│──────────▶│ RESIDENT (<repo>.<wave>) │
  │ pens · folds · doors  │           │ wave runner · queue │
  │ observer · supervisor │◀──deltas──│ GOAL.md seed · schedule  │
  └──────────┬────────────┘           └────────────▲─────────────┘
             └────────── /events?inbox=true ───────┘
```

- **The listener** is the channel made durable — hear / check / fold / tell,
  **vendor-free**. It holds the wave's one journal pen (work-line channels are
  bus addresses and journal nothing), serves the doors, folds the store's
  worker observations and its hands' bus broadcasts, keeps the registry seat
  and the discovery pointer, and supervises the resident. It serves from the
  **origin repo** and creates no worktrees.
- **The resident** is the wave's Loop (see `flowloop/wave.rs`): the durable
  playhead selects one flow step, starts one live harness session as that
  step's body, then advances only when the body completes or the user skips.
  Continuity is the journaled thread, playhead, GOAL.md, and memory. It
  bootstraps and enters the wave's `<repo>.<wave>` sibling worktree — passes
  never run in the main checkout. Its input is its own wave's
  `/events?inbox=true` subscription — the SSE thread stream `lf chat --follow`
  follows, plus the inbox scope, and nothing to do with `lf radio sub`, which polls
  the bus table and never opens a socket. Its
  output is ordered turn deltas through the token-gated resident door. It
  never touches a journal file — the single writer stays with the listener.

The listener spawns the resident by name — `lf __resident <name>` — carrying
private endpoint/token environment. The two processes are runtime plumbing, not
two user-facing modes, and the hidden subcommand keeps them that way.

An earlier design ran both halves from one `lf loop <name>` command, choosing
which half to be by checking whether the endpoint and token were present in the
environment. Any process that inherited a parent wave's environment — a tmux
child, a promoted subwave — then booted the wrong half by accident. Serving is
now `lf serve`, and the body is `lf __resident`.
Environment configures a process; it no longer decides what the process is.

`lf stop <name>` posts to the listener's lifecycle door. The listener stands
down its supervisor, terminates the resident, deregisters its session, and
removes this boot's discovery files. Durable Task Sessions are separate tmux
sessions and keep running.

- **One Loop, one thread.** Chat and progress share the same context. A
  message while idle reaches the next body. A steer while a compatible
  harness is active is injected with `send_input`; unsupported harnesses
  queue it for the next body. The resident declares what it consumed in its `TurnOpened`
  delta's `answers`, and the listener validates against its pending fold
  before journaling `TurnStarted.answers`; live inputs append
  `TurnSteered.answers`. With no explicit continuation queued, the default
  `wave` flow advances continuously and wraps to its next iteration.
- **Crons are the third deadline.** `crons: [{flow, schedule}]` in the
  wave's `GOAL.md` frontmatter (re-read live, no restart); a due schedule
  while idle opens a system pass — "cron due: <flow> — run it" — and
  the loop responds with judgment. Mid-pass due dates fire at the
  boundary; occurrences older than 24h are missed, not replayed. The
  daemon's cron poller and `wave_crons` table died in the collapse.
- **The Wave coordinates; Task Sessions ship.** Each pass's seed is the rendered
  `GOAL.md` plus the coordinating-session discipline. The Wave creates or
  selects a Linear task, starts it with `lf task run <issue-id>`, and remains
  available while that Task Session works in its immutable sibling worktree.
  Structured Task commands and events carry steering and results; raw terminal
  bytes and child tool chatter do not become the orchestration protocol.
- **Interrupts stop the active harness.** The resident sends an `Interrupting`
  state delta, stops the body, and closes the turn
  (`TurnFinished{interrupted}`); non-empty interrupt text queues for the
  next pass. The listener keeps its own 20s janitor for a resident gone
  fully silent: past it the open turn force-finalizes server-side and late
  wire deltas for it are dropped until the next `TurnOpened` — the
  anti-wedge lives where the pen is.
- **Failure is a dead process.** Three consecutive failed passes (nonzero
  exit, spawn failure, timeout) → the resident reports `LoopState::Failed`
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
  boot's pid probe. A second `lf serve` refuses to start naming the live
  session unless `--force` takes over. A wave the store has never seen gets
  its row created at boot. No registry store on the machine → warn once, run
  fully functional, with one file-level floor: a `.wave-endpoint` that
  answers `GET /health` for this wave also refuses a second server.

Truth is the per-wave append-only journal —
`.lf/journal/waves/<name>/journal.jsonl` under the **origin (main) repo**,
per-machine, never committed. One journal per served mind, zero per channel: a
journal buys delivery to a subscriber who was absent at publish time, which is
a need minds have and topics never do. A work line's report reaches the wave as
one attributed copy in *this* journal, folded off the bus; nothing is written in
its worktree. The in-process state (`WaveRuntime`) is a fold
of it: the `thread` and the loop state are rebuilt from the journal on
boot, so a restart keeps the full conversation and turn ids continue
monotonically. The journal event vocabulary predates the pass model —
`TurnStarted`/`TurnItem`/`TurnSteered`/`TurnFinished`/`LoopState` and
`PlayheadChanged` are journaled by the listener on receipt from the wire.
`wave/<name>/MEMORY.md`
seeds the loop (read from the origin repo), and the live listener holds
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
  cost_usd}`, `turn_steered {answers}`, `loop_state {to, reason}`
  (interrupting/failed — turning/idle are derived from the turn deltas).
  Sent serially, so per-turn order is the transport's order; turn ids never
  ride the wire — the listener mints them from its journal seq. Plus
  `POST /resident/attach {pid}` (liveness registration + revival; returns
  `{wave}`) and `GET /resident/context` (`{in_flight}`; serving it freshens
  the store fold).
- **Listener → resident**: `GET /events?inbox=true` adds `inbox` SSE frames
  to the primary subscription — `{kind: "message", id, op, text, from}` per
  resident-directed op, the pending queue replayed on connect. Bare interrupts
  and skips are live-only control frames (`{kind: "interrupt"}`) carrying no
  id, because nothing is journaled. The default `/events` stream is
  byte-identical to the pre-resident wire.
- **Auth (stopgap)**: the resident door requires this boot's token in the
  `x-lf-resident-token` header. The listener generates it at bind, passes it
  to a spawned resident via `LF_WAVE_RESIDENT_TOKEN`, and publishes it at
  `wave/<name>/.wave-resident-token` (owner-only) for attached residents —
  the same filesystem-trust domain as the endpoint pointer. When a human or
  a remote loop can hold the seat, the token becomes a gatekeeper-issued
  credential.

## Two wires: the bus and the thread

**The bus** is the `bus_messages` table in the shared store — nothing else.
`lf radio pub` publishes with an INSERT; every subscriber polls forward from an id
cursor. No server is in the path, so publishing works with zero loopflow
processes running and two detached hands hear each other with no wave awake. A
sweeper drops rows past a one-hour wall-clock window — on every publish, and on
every read, so a bus that went quiet still forgets on schedule: the bus is a
wire, not a log. Channel names are addresses (`goals`, `goals.148e0e02`), dots
are the tree, and subscription is by prefix. `lf radio sub [CHANNEL] [--json]` tunes
in — you hear what is said while you listen, and nothing published before you
tuned in replays.

The byline is testimony and the channel is evidence. With no server in the
path, client-submitted attribution is the only kind possible: `lf radio pub`
derives its byline from the ambient identity it already resolves for routing
(`LFD_CHANNEL`, else `LFD_WAVE_ID`, else the worktree name), `--from` overrides
it, and the row carries both. A forged byline is not prevented — it shows up as
a mismatch with the channel it arrived on.

A served mind is just another subscriber ([`bus.rs`]). Its `BusListener` polls
its family's channels from a **durable** cursor, records what it hears as an
attributed copy in its own journal, and wakes its loop. So a mind asleep when a
hand reported catches up on wake, and a clean restart replays nothing. Delivery
precedes the cursor commit, so the guarantee is at-least-once: a crash between
journaling a report and committing the cursor re-reads that one row on the next
boot. Beyond the sweep window the report is gone, and the cursor jump is
announced in the thread rather than swallowed — `bus_messages` is
`AUTOINCREMENT`, so the high-water mark outlives the rows and the miss stays
visible even on a bus swept empty. The PR and `lf runs` remain the records of
record. A mind never wakes itself: rows bylined with its own channel are read
and skipped.

**The thread** is the human surface — journaled, durable, replayed — and it
stays SSE on the listener. `lf chat` resolves its target wave from context
(`LFD_CHANNEL`, else `LFD_WAVE_ID`, else the worktree name; `--parent` walks
`parent_wave_id` through the registry; `--wave <name>` is explicit, a dotted
name resolving to its family head), then finds that wave's live endpoint via
its WaveAgent session row (falling back to `.wave-endpoint`). `--steer` POSTs
`steer`, which reaches a live steer-capable turn and otherwise queues. Bare `lf
chat` POSTs an unattributed human `message`. With no wave
context anywhere, `lf chat` and `lf memory` writes exit 0 with one stderr note.
A resolvable wave whose server is down errors instead (mail to a dead wave
bounces, it doesn't vanish).

`lf chat --follow` reads that stream and writes into it from one pane. The
resident is the subscription's second customer — same machinery, plus the inbox scope.
Task Sessions carry the owning Wave identity explicitly and report structured
progress through their Task event stream.

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
| `GET /health`             | `{status, loop_state, wave, turns, workers, paused, uptime_seconds}`; `status` is channel liveness; `loop_state` is `idle \| turning \| interrupting \| failed`; `workers` counts observed in-flight worker runs |
| `GET /conversation`       | `{turns: [Turn]}` — the whole thread; `?limit=N` tails the last N turns (open turn included) |
| `GET /events`             | SSE, the served mind's thread. No channel scoping — agent broadcast is the bus, not this door. Event names: `state` (loop-state name), `playhead` (durable cursor snapshot), `turn` (replay then live; repeated ids replace), `op` (live worker motion), `memory-add` (replay then live), `memory` (live curation summaries), and — only with `?inbox=true` — `inbox` (pending replay + live controls). |
| `POST /messages {op, text}` | Human thread input. `op` required: `message` (next body), `steer` (inject into the active steer-capable harness, otherwise queue), or `interrupt` (stop the active body; non-empty text queues for the retry). `say` and bylines are rejected; machine speech uses `lf radio pub`. Returns `{turn, state}`. |
| `GET /playhead`           | Durable invocation stack, active body, `now`, `next`, local queue, and return target. |
| `POST /playhead/enqueue {flow}` | Enqueue a flow FIFO at the innermost invocation and return the updated playhead. |
| `POST /playhead/skip`     | Stop and skip the current body, or advance a failed idle step, without destroying its route. |
| `GET /memory`             | `{content}` — the wave's MEMORY.md (origin repo). |
| `GET /memory/log`         | `{facts}` — add-stream facts since the last curation, oldest first. |
| `POST /memory {op, content, summary}` | `op`: `update` replaces `MEMORY.md`; `add` publishes one replayable fact. `summary` null → first non-empty content line. Returns `{summary}`. |
| `POST /resident/attach {pid}` | Resident door (token-gated): register the resident's pid, revive a failed loop. Returns `{wave}`. |
| `POST /resident/deltas {deltas}` | Resident door (token-gated): ordered turn deltas → the journal fold. Returns `{accepted}`. |
| `GET /resident/context`   | Resident door (token-gated): `{in_flight, playhead}`; freshens the store observations. |

### Turn

```json
{
  "id": "turn-3",
  "role": "user | assistant",
  "text": "…",
  "status": "pending | running | completed | failed | interrupted",
  "items": [ ConversationItem, … ],
  "created_at": "2026-07-04T00:42:03.412861Z",
  "from": "worker",
  "body": null
}
```

`items` are the tool/command/file/message artifacts a pass produced, in
order (`ConversationItem` — see `chat/types.rs`). User turns
carry empty `items`. Turn `id`s are a single monotonic `turn-<n>` sequence
across all sources. `from` is the speaker byline of an attributed emission
(`lf radio pub`); null for the loop's own turns and plain user turns.

## Demo

```
lf serve demo
# → lf serve · demo · listener on http://127.0.0.1:52306 · spawning resident (Ctrl-C to stop, …)
# → lf serve · demo · resident · listener http://127.0.0.1:52306 · worktree …/demo-repo.demo

curl 127.0.0.1:52306/health
curl -X POST 127.0.0.1:52306/messages -H 'content-type: application/json' \
     -d '{"op":"message","text":"status?"}'
curl 127.0.0.1:52306/conversation
curl -N 127.0.0.1:52306/events
lf stop demo
```

The full guided walk — chat, steer, interrupt, Task Sessions, attributed
reports, restart, teardown — is `scripts/demo_wave.sh` (`--smoke` for a
zero-model-turn sanity pass). The two-process topology has its own ignored
live test, `cargo test -p loopflow --test wave_live_smoke -- --ignored`.
