# Claude Adapter — Design Review

## What was implemented

Added Claude as a second session provider in `lfd`, running through the same `/v0/sessions` API and normalized event schema as Codex. Claude sessions use process-per-turn invocation (`claude -p ... --output-format stream-json`) with `--resume` for turn continuity.

Key additions:
- `ClaudeAdapter` in `adapter/claude.rs` (828 lines including tests)
- NDJSON stream parser mapping Claude events → normalized `SessionEvent`
- Store methods for persisting `provider_session_id` (sqlite + postgres)
- `supports_provider` and `default_create_adapter` dispatch for both providers
- Style guide updates banning factory patterns and reshaping production code for tests

## Key choices

**Process-per-turn, not persistent.** Claude `-p` exits after each turn. Continuity comes from `--resume <session_id>`. This means `start()` validates the binary and `send_input()` spawns a fresh process each turn. Simpler than maintaining a persistent subprocess with custom IPC.

**Single in-flight turn guard.** `AtomicBool` + `compare_exchange` rejects concurrent `send_input()` calls. This preserves deterministic event ordering without needing a queue.

**Tool name → item type inference.** Claude provides tool names, not typed categories. The adapter maps: `Bash` → `Command`, `Edit`/`Write`/`NotebookEdit` → `File`, everything else → `Tool`. This is the same categorization Claude Code uses internally.

**ProviderSessionId as internal event.** The bridge task in `SessionManager` intercepts `ProviderSessionId` events and persists them to the DB rather than forwarding to SSE clients. This keeps the provider session ID durable for `--resume` without polluting the client event stream.

**No factory traits.** The adapter dispatch is a function pointer (`CreateAdapterFn`), not a trait or registry. Tests use a plain function that returns a `FakeAdapter`.

## How it fits together

```
Client → POST /sessions {provider: "claude"} → SessionManager
  → default_create_adapter("claude", event_tx) → ClaudeAdapter
  → bridge task: adapter events → persist + broadcast to SSE

Client → POST /sessions/{id}/input → SessionManager.send_input()
  → ClaudeAdapter.send_input()
    → spawns `claude -p <content> --output-format stream-json [--resume <id>]`
    → reader task: NDJSON stdout → process_line() → SessionEvent
    → bridge task persists each event
```

The adapter and session manager are fully decoupled through the `broadcast::Sender<SessionEvent>` channel. The bridge task is the only component that knows both the adapter's raw events and the persistence layer.

## Risks and bottlenecks

- **Process spawn latency.** Each `send_input()` spawns a Claude process. First turn includes Claude's initialization overhead. Acceptable for now; process pooling would add complexity with unclear benefit.
- **No streaming of tool stdout.** Claude returns tool results after completion, so command output arrives in one batch via `ItemCompleted`. Live streaming of `Bash` output would require Claude to change its output format.
- **Accumulated input JSON parsing.** `input_json_delta` chunks are concatenated as strings and parsed at tool completion. Malformed partial JSON from a crash would be silently dropped (parsed with `.ok()`).
- **Race between reader task and stop.** `stop()` kills the child process and aborts the reader task. If the process exits normally between the kill and the abort, the reader may emit a `TurnCompleted(Completed)` simultaneously with the stop flow. The `AtomicBool` guard prevents state corruption but the event ordering could have a stale completed event followed by a stop.

## What's not included

- **Diff synthesis.** Claude doesn't emit a turn-level diff like Codex's `turn/diff/updated`. `DiffUpdated` events won't appear for Claude sessions.
- **Incremental tool output streaming.** `ItemUpdated` deltas aren't emitted for Claude tool executions.
- **Concerto UI integration.** The UI hasn't been wired to create or consume Claude sessions yet.
- **Restart rehydration.** Active Claude sessions are still lost on daemon restart (same gap as Codex).
