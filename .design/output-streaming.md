# Output Streaming

Stream live task output through lfd to Maestro and other subscribers.

## Status

Complete. Both Python and Swift implementations done.

**Python:**
- `server.py`: Added `output.line` handler that broadcasts to subscribers
- `collector.py`: Added `_send_output_line()` that streams each formatted line
- `api.md`: Documented the new method and event

**Swift (Maestro):**
- `LFDEventService.swift`: Extended to parse `session.*` and `output.line` events
- `AppState.swift`: Added `liveOutputBySession` and `activeSessionIds` state
- `OutputPanel.swift`: New collapsible panel showing live task output
- `ContentView.swift`: Integrated panel below PromptLauncher

## Why this matters

This branch documented the lfd socket protocol and event system. The events are lightweight metadata: `session.started`, `session.ended`, `agent.*`. What's missing is the *content* - what the agent is actually doing.

Right now:
- Collector writes output to `~/.lf/logs/<worktree>/<session>.log`
- Maestro can see "implement: running" but not what it's reading/writing
- You have to open a terminal and tail the log file to watch

With output streaming:
- Maestro shows live output as tasks run
- No terminal juggling - watch multiple worktrees from one window
- The documented API becomes actually useful for observability

## Design

### New event type: `output.line`

```json
{
  "event": "output.line",
  "data": {
    "session_id": "uuid",
    "text": "→ Read: src/config.py",
    "timestamp": "2025-01-14T10:30:15Z"
  }
}
```

The `text` field contains already-formatted output (the collector already has `_format_stream_line` that parses JSON events into human-readable lines like `→ Read: foo.py`).

### Flow

```
Agent CLI → collector → lfd socket → subscribers (Maestro)
                     ↘ log file (existing)
```

The collector currently writes to:
1. Log file (plain text, formatted)
2. JSON log file (raw agent events)

Add a third destination:
3. lfd socket via `output.line` events

### Collector changes

In `collector.py`, modify `_run_streaming()` to send each formatted line to lfd:

```python
def _send_output_line(session_id: str, text: str) -> None:
    """Send output line to lfd. Fire-and-forget."""
    _send_fire_and_forget("output.line", {
        "session_id": session_id,
        "text": text,
    })

# In the output loop:
for formatted in formatted_lines:
    write_log_line(log_file, formatted)
    if foreground:
        print(formatted, flush=True)
    _send_output_line(session_id, formatted)  # NEW
```

### Server changes

In `server.py`, add a pass-through handler:

```python
async def _handle_output_line(self, params: dict) -> Response:
    """Accept output lines and broadcast to subscribers."""
    session_id = params.get("session_id")
    text = params.get("text")

    if not session_id or not text:
        return error("Missing parameters")

    await self._broadcast(Event("output.line", {
        "session_id": session_id,
        "text": text,
        "timestamp": datetime.now().isoformat(),
    }))
    return success({})
```

The existing broadcast/subscribe machinery handles routing to the right clients.

### Maestro changes

In `LFDEventService.swift`, extend `WorktreeEvent` to handle output:

```swift
enum LFDEvent: Sendable {
    case worktree(WorktreeEvent)
    case output(session: String, text: String)
}
```

Add a view that shows streaming output for a selected session.

## What's NOT in scope

- **Backpressure/buffering**: Fire-and-forget is fine. If lfd is slow or dead, output still goes to log files. Missing a few lines in the UI is acceptable.

- **Output history via socket**: Historical output should be read from log files. The socket is for live streaming only.

- **Filtering by worktree at daemon**: Maestro subscribes to `output.*` and filters locally. Simpler than adding worktree-scoped subscriptions.

## Implementation order

1. Add `output.line` handler to server.py
2. Add `_send_output_line` calls to collector.py
3. Update Maestro to display streaming output
4. Add docs to api.md

## Open questions

None - this is a straightforward extension of the existing event system.
