# wave — the reactive wave server

`lf wave <name>` starts a long-lived server for a wave. It is **not** a loop: it
stays up until stopped and reacts to two independent event sources — neither
blocks the other.

```
                ┌─ subagent progress events ──┐
  Wave server ──┤                             ├──▶ react
                └─ user messages ─────────────┘
```

- **Progress** (autonomous): the [`progress`] arm keeps a `codex exec --json`
  subagent grinding. Every finalized turn is narrated into the thread and folded
  into `wave/<name>/MEMORY.md`.
- **Chat** (over HTTP): a user message is answered **talk-only** from memory and
  current progress state. Chat observes; it does not steer progress.

All state is in-process (`WaveRuntime`): the `thread` the user sees, a MEMORY
handle, an inbox channel, and a supervisor tracking every live subagent run.
No files are used as IPC.

## Wire contract (snake_case, stable)

The server binds a loopback port. Concerto finds it via the discovery pointer:

```
wave/<name>/.wave-endpoint   →   127.0.0.1:<port>     (address only; removed on shutdown)
```

| Method + path             | Behavior |
|---------------------------|----------|
| `GET /health`             | `{status, wave, turns, subagents, uptime_seconds}` |
| `GET /conversation`       | `{turns: [Turn]}` — the whole thread |
| `GET /conversation/stream`| SSE; each event named `turn`, `data:` a `Turn` JSON. Replays the thread on connect, then streams live. |
| `POST /messages {text}`   | Appends a user `Turn` and returns it. The reply lands as a later `assistant` turn. |

### Turn

```json
{
  "id": "turn-3",
  "role": "user | assistant",
  "text": "…",
  "status": "in_progress | completed | failed",
  "items": [ ConversationItem, … ],
  "created_at": "2026-07-04T00:42:03.412861Z"
}
```

`items` are the tool/command/file/message artifacts a subagent produced, in
order (`ConversationItem` — see `lfd/conversations/types.rs`). User and reply
turns carry empty `items`. Turn `id`s are a single monotonic `turn-<n>` sequence
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
