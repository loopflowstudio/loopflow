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
  work is dispatched to subagents (`lfq worker run`). The server tails lfd's
  event stream and journals `WorkerDispatched`/`WorkerFinished` observations
  (it is never in the dispatch path); dispatched-not-finished workers ride
  each heartbeat as a compact `<in_flight>` section.
- **Failure is bounded.** A failed turn returns the mind to idle; three
  consecutive failures (or a dead vendor session) mark the mind `failed` and
  stop the heartbeat. The next user message revives it.
- **One brain per wave.** On boot the server registers with a running lfd as
  the wave's `WaveAgent` session (`POST /v0/waves/{wave}/agent/register`,
  source `wave_server`; deregistered on shutdown, pid-probed by lfd's
  reconciliation after a crash). lfd's loop ticker and `run_wave` skip a
  wave with a live registered brain; a second `lf wave` refuses to start
  unless `--force` takes over. lfd unreachable → warn once, run fully
  functional, retry lazily; no daemon means no enforcement (status quo).

Truth is the per-wave append-only journal —
`.lf/journal/waves/<name>/journal.jsonl`, per-machine, never committed. The
in-process state (`WaveRuntime`) is a fold of it: the `thread` the user sees,
the mind state, and the vendor thread id are rebuilt from the journal on
boot, so a restart keeps the full conversation and turn ids continue
monotonically. The vendor thread itself cold-starts on codex (the app-server
driver takes no resume id); the new `ThreadStarted` is journaled so the break
is explicit. `wave/<name>/MEMORY.md` is read-only here (seeds the mind); the
journal carries the raw history. The journal is server-owned persistence, not
IPC.

The mind runs in the repo root the server was started from — run `lf wave`
from the wave's worktree. Main-checkout protection and worktree bootstrap
are still to come.

## Wire contract (snake_case, stable)

The server binds a loopback port. Concerto finds it via the discovery pointer:

```
wave/<name>/.wave-endpoint   →   127.0.0.1:<port>     (address only; removed on shutdown)
```

| Method + path             | Behavior |
|---------------------------|----------|
| `GET /health`             | `{status, wave, turns, subagents, uptime_seconds}`; `status` is the mind state: `idle \| turning \| interrupting \| failed` |
| `GET /conversation`       | `{turns: [Turn]}` — the whole thread |
| `GET /conversation/stream`| SSE; each event named `turn`, `data:` a `Turn` JSON. Replays the thread on connect, then streams live. |
| `POST /messages {text}`   | Appends a user `Turn` and returns it. The mind answers it at its next turn boundary. |

### Turn

```json
{
  "id": "turn-3",
  "role": "user | assistant",
  "text": "…",
  "status": "pending | running | completed | failed | interrupted",
  "items": [ ConversationItem, … ],
  "created_at": "2026-07-04T00:42:03.412861Z"
}
```

`items` are the tool/command/file/message artifacts the mind produced, in
order (`ConversationItem` — see `lfd/conversations/types.rs`). User turns
carry empty `items`. Turn `id`s are a single monotonic `turn-<n>` sequence
across all sources.

## Demo

```
lf wave demo
# → lf wave · demo · reactive server on http://127.0.0.1:52306 (Ctrl-C to stop)

curl 127.0.0.1:52306/health
curl -X POST 127.0.0.1:52306/messages -d '{"text":"status?"}'
curl 127.0.0.1:52306/conversation
curl -N 127.0.0.1:52306/conversation/stream
```
