# Conversations

The reusable stream → conversation-turn engine, plus the per-wave chat server it
feeds. There is no central conversation daemon — `lf wave <name>` hosts the chat
server in-process (see `server.rs`).

## Layers

- **`harness/` + `types.rs`** — vendor stream engines (codex/claude/opencode) that
  map raw agent output into `ConversationItem`s and `ConversationEvent`s.
  `harness/conformance_tests.rs` pins the mapping against captured traces under
  `harness/testdata/`.
- **`turns.rs`** — folds a live `StreamEvent` sequence (from `engine::stream`,
  which normalizes `codex exec --json`) into `ChatTurn`s.
- **`server.rs`** — the per-wave HTTP chat server Concerto observes.

## Chat server

`lf wave <name>` binds an ephemeral loopback port and publishes it to
`wave/<name>/.chat-endpoint` (`127.0.0.1:<port>`, one line). Concerto reads that
file to find the server.

```bash
PORT=$(cat wave/<name>/.chat-endpoint)

curl "http://$PORT/health"           # { "status", "wave", "pass", "turns" }
curl "http://$PORT/chat"             # { "wave", "turns": [ChatTurn, …] }
curl -N "http://$PORT/chat/stream"   # SSE: replay current turns, then live

curl -X POST "http://$PORT/chat" \
  -H 'Content-Type: application/json' \
  -d '{"text":"also check the tests"}'
```

`GET /chat/stream` is Server-Sent Events. Each event is named `turn`; its `data`
is one `ChatTurn` JSON object. The stream replays the current turns on connect,
then emits new/updated turns live (an in-progress turn is re-sent as its text
grows).

`POST /chat` appends the message to `wave/<name>/MAILBOX.md` and records a `user`
turn. The next `lf goal --once` pass folds the mailbox into its prompt under an
`<lf:mailbox>` tag, then clears the file — each message is delivered once.

### ChatTurn shape

```json
{
  "id": "turn-1",
  "role": "assistant",            // "user" | "assistant"
  "text": "Ran the tests, all green.",
  "status": "completed",          // "in_progress" | "completed" | "failed"
  "items": [                      // tool/command/file/message items, in order
    { "type": "command", "id": "item-0", "command": ["cargo test"], "cwd": "", "status": "completed" }
  ],
  "created_at": "2026-07-03T22:14:05Z"
}
```

`user` turns use ids `user-<n>` and are always `completed` with no items.
Assistant turns use ids `turn-<n>`.
