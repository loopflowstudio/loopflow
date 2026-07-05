# wave — the reactive wave server

`lf wave <name>` starts a long-lived server for a wave. It is **not** a loop:
its brain is **the mind** — one persistent codex app-server thread, driven
through the conversations harness — and turns are scheduled by events, never
a busy-loop.

```
                ┌─ user messages (HTTP inbox) ─┐
  Wave server ──┤                              ├──▶ the mind (one vendor thread)
                └─ heartbeat when idle ────────┘
```

- **One mind, two inputs.** Chat and progress share the same context. A
  message while idle starts a turn immediately; messages during a turn queue
  (append-and-coalesce, never rejected) and one boundary turn drains them
  all — its journal `TurnStarted.answers` names every consumed message. Quiet
  for 5 minutes with an empty queue → a heartbeat turn nudges the next
  orchestration step.
- **The mind orchestrates, never grinds.** Its operating prompt is the
  rendered `GOAL.md` seed plus the coordinating-session discipline; heavy
  work is dispatched to subagents (`lf q worker run <wave> --flow F --task T`,
  daemonless — run + session rows written straight to the shared store, the
  worker launched in a detached tmux session). The server polls the same
  store and journals `RunObserved`/`RunCompleted` observations (it is
  never in the dispatch path) — every ~10s and once before each mind turn;
  dispatched-not-finished workers ride each heartbeat as a compact
  `<in_flight>` section.
- **Failure is bounded.** A failed turn returns the mind to idle; three
  consecutive failures (or a dead vendor session) mark the mind `failed` and
  stop the heartbeat. The next user message revives it.
- **One brain per wave.** On boot the server writes itself a `WaveAgent`
  session row in the shared store (source `wave_server`, endpoint + pid in
  `env`; the db IS the registry — no daemon in the path; opening the store
  applies pending migrations, so an `lf` built ahead of the machine's lfd
  never hits schema drift). The row is marked terminal on shutdown or any
  termination signal — the interrupt hooks cover SIGINT, SIGTERM, *and*
  SIGHUP (`tmux kill-session` delivers SIGHUP; it must deregister the row,
  remove the pointer, and kill the mind's codex process group, same as
  Ctrl-C). A crashed server's row is closed by the
  next boot's pid probe (and by lfd's reconciliation, when one runs). lfd's
  loop ticker and `run_wave` skip a wave with a live registered brain; a
  second `lf wave` refuses to start naming the live session unless `--force`
  takes over (kill by recorded pid, cancel the row). A wave the store has
  never seen gets its row created at boot — a reachable store always means a
  registered server. No registry store on the machine → warn once, run fully
  functional, with one file-level floor: a `.wave-endpoint` that answers
  `GET /health` for this wave also refuses a second server (`--force`
  overwrites); shutdown removes the pointer only while it still holds the
  server's own address.

Truth is the per-wave append-only journal —
`.lf/journal/waves/<name>/journal.jsonl` under the **origin (main) repo**,
per-machine, never committed. The
in-process state (`WaveRuntime`) is a fold of it: the `thread` the user sees,
the mind state, and the vendor thread id are rebuilt from the journal on
boot, so a restart keeps the full conversation and turn ids continue
monotonically. The vendor thread itself cold-starts on codex (the app-server
driver takes no resume id); the new `ThreadStarted` is journaled so the break
is explicit. `wave/<name>/MEMORY.md` seeds the mind, and the live server
holds its pen: `lf memory update`/`add` POST to the memory routes, which
write the origin file and journal `MemoryUpdated` — the journal carries the
raw history. The journal is server-owned persistence, not IPC.

## Channels

A CHANNEL is a named stream — journal + thread + subscribability. The wave's
own channel is its name (`goals`), journal under the origin repo. A work
line's channel is the ownership name — exactly the worktree basename minus
the repo prefix (`goals.148e0e02`) — journal IN that worktree
(`.lf/journal/waves/<channel>/journal.jsonl`): it travels with the branch
and dies with it (channels are conversations, not records). The server
serves the whole FAMILY: it holds the pen for every child journal (all pens
in one process), folds each channel separately, and the family view folds
upward. Child channels have no mind and no memory — pure streams. Names are
topics, dots are the tree; subscription is by name or prefix. `lf q worker
run` mints the work line's channel with its worktree — journal initialized
there, `LFD_CHANNEL` in the worker's env, and one `POST /channels` knock so
the wave's thread shows "work line <name> opened".

The speech surface is `lf chat` — the same verb for minds, workers, humans,
and scripts (the one-door exec convention). It resolves its target CHANNEL
from context (`LFD_CHANNEL`, else `LFD_WAVE_ID`, else the worktree name —
inside a work-line worktree that is the work line's own channel: speak
locally; `--parent` walks `parent_wave_id` through the registry; `--wave
<name>` is explicit, dotted names addressing a work line through its family
head), finds the FAMILY HEAD's live endpoint via its WaveAgent session row
(falling back to `.wave-endpoint`), and POSTs a `say` op with the channel
field. Publish-to-no-subscriber drops: with no wave context anywhere,
`lf chat` and `lf memory` writes exit 0 with one stderr note — the
vocabulary is safe in every prompt unconditionally. A resolvable wave whose
server is down errors instead (mail to a dead wave bounces, it doesn't
vanish — child channels included: the pen is the parent's). Dispatched
workers are instructed to finish with an `lf chat` report, closing the loop
through the same door — it arrives in the thread with a `from` byline (label
from `LFD_AGENT_ROLE`, session id from `LFD_SESSION_ID`) and wakes the mind
like any input.

`lf sub [NAME] [--json]` is the read half: follow the family's `/events`
stream (turns, mind state, memory) until killed, reconnecting with backoff
and re-resolving the endpoint across server restarts. `lf sub goals` follows
the whole family; `lf sub goals.148e0e02` follows one work line. Same
targeting and drop semantics as `lf chat` — outside any wave it exits 0 with
one note.

The context flows back out the same way: every `lf` run born inside a wave
inherits ambient context by CHANNEL in its assembled prompt —
`<lf:wave-chat-recent>` (the newest turns, hard-capped) and
`<lf:wave-memory>` (the WAVE's MEMORY.md — memory is wave identity; work
lines have none). At the wave home the chat section is the wave channel; in
a work-line worktree it reads down the tree — the work line's own thread
plus, compactly, the parent wave channel's recent turns. Reads go through
the live server, else a read-only journal fold, else nothing; no wave
resolves → both sections are absent and flows stay wave-agnostic (see
`engine/wave_context.rs`).

`lf wave <name>` self-bootstraps its worktree: it ensures the wave's
`<repo>.<wave>` sibling exists (creating it off main on first boot) and
enters it before starting the server, so the mind always runs there — never
the main checkout. Wave state (the journal under `.lf/journal/`, the
`wave/<name>/.wave-endpoint` pointer, MEMORY.md) deliberately stays under
the origin repo, not the worktree: it survives worktree pruning, and
Concerto and a restarted server agree on where it is.

## Wire contract (snake_case, stable)

The server binds a loopback port. Concerto finds it via the discovery
pointer, under the origin repo's `wave/<name>/`:

```
wave/<name>/.wave-endpoint   →   127.0.0.1:<port>     (address only; removed on shutdown)
```

| Method + path             | Behavior |
|---------------------------|----------|
| `GET /health`             | `{status, mind, wave, turns, workers, uptime_seconds}`; `status` is channel liveness — always `serving` while the process answers; `mind` is the resident's state (`idle \| turning \| interrupting \| failed`), null for a channel with no resident — a live channel whose mind died reads `serving` + `failed`; `workers` counts observed in-flight worker runs |
| `GET /conversation`       | `{turns: [Turn]}` — the whole thread; `?limit=N` tails the last N turns (open turn included) |
| `GET /events`             | SSE, the family's one unified stream. Scope: `?channel=<name>` (one channel), `?prefix=<name>` (subtree), default = whole family; names outside the family 404. Three event names: `state` (`data:` the mind-state name, sent on subscribe and on every transition — primary channel only), `turn` (`data:` a `Turn` JSON; replay on connect, then live; a child channel's turns carry an extra `"channel"` key, the wave's own ride untagged; ids repeat — and repeat across channels — each frame replaces the client's previous state for that (channel, id)), and `memory` (`data:` the `MemoryUpdated` summary string, fired on every curation; live-only — MEMORY.md is the durable state; primary only). |
| `POST /messages {op, text, from?, channel?}` | `op` required: `message` (queued; the next turn answers it), `steer` (into the live turn when supported), `interrupt` (cancel the open turn; non-empty text becomes the next turn), or `say` (an attributed emission — `lf chat`; `from {session_id?, label}` required for `say`, rejected otherwise). `channel` null = the wave channel; a child name lands in that work line's journal (404 outside the family or when its worktree is gone; no resident there — steer degrades, bare interrupt no-ops). Returns `{turn, state}`. |
| `POST /channels {name, run_id}` | The dispatch knock: journals `ChannelOpened` on the wave channel so the thread shows "work line <name> opened". Idempotent on `run_id` (`{turn: null}` on a repeat); 404 outside the family. Returns `{turn}`. |
| `GET /memory`             | `{content}` — the wave's MEMORY.md (origin repo). |
| `POST /memory {op, content, summary}` | `op`: `update` (full replacement) or `add` (append one bullet). `summary` null → first non-empty content line. The server holds MEMORY.md's pen and journals `MemoryUpdated`. Returns `{summary}`. |

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

`items` are the tool/command/file/message artifacts the mind produced, in
order (`ConversationItem` — see `lfd/conversations/types.rs`). User turns
carry empty `items`. Turn `id`s are a single monotonic `turn-<n>` sequence
across all sources. `from` is the speaker byline of an attributed emission
(`lf chat`); null for the mind's own turns and plain user turns.

## Demo

```
lf wave demo
# → lf wave · demo · reactive server on http://127.0.0.1:52306 (Ctrl-C to stop, RUST_LOG=loopflow=debug for the firehose)

curl 127.0.0.1:52306/health
curl -X POST 127.0.0.1:52306/messages -H 'content-type: application/json' \
     -d '{"op":"message","text":"status?"}'
curl 127.0.0.1:52306/conversation
curl -N 127.0.0.1:52306/events
```

The full guided walk — chat, steer, interrupt, worker dispatch, attributed
reports, restart, teardown — is `scripts/demo_wave.sh` (`--smoke` for a
zero-model-turn sanity pass).
