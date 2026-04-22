# Sessions

```bash
curl -N "$LFD_URL/v0/sessions/$SESSION_ID/events?after_seq=42"
curl -X POST "$LFD_URL/v0/sessions/$SESSION_ID/input" \
  -H 'Content-Type: application/json' \
  -d '{"text":"also check the tests"}'
```

`GET /v0/sessions/{id}/events` streams persisted `SessionEvent`s and replays anything after `after_seq`. Reconnect with the last SSE `id` to catch up without losing events.

`POST /v0/sessions/{id}/input` sends text into a live session. Codex sessions steer the running turn when one is active and start a new turn when idle. Tool approvals are not part of this endpoint; tools continue to auto-approve.

Session DTOs include `input_supported`:

| Value | What it means |
|-------|---------------|
| `true` | Show the input field and call `POST /input` |
| `false` | Disable input; the harness cannot accept session input yet |

Codex reports `input_supported: true`. Claude and OpenCode report `false`; posting input to them returns a 4xx error with `input not supported for this harness`.
