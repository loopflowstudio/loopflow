---
status: todo
phase: 2
---
# OpenCode Stream Parser

Parse opencode's NDJSON streaming output into `StreamEvent` types.

## Current

`StreamParser::feed_line` dispatches by `"type"` field:
- **Claude**: `"assistant"` → text/tool_use blocks, `"result"` → cost/duration
- **Codex**: `"item.started"` / `"turn.completed"` → items, turns
- **Gemini**: `"message"` / `"tool_use"` → text, tools

## OpenCode's NDJSON Format

`opencode run --format json` emits one JSON object per line. Each event has:

```json
{
  "type": "text",
  "timestamp": 1759406015783,
  "sessionID": "ses_65b3acf58ffe...",
  "part": {
    "id": "prt_9a4c543c6001...",
    "messageID": "msg_9a4c53490001...",
    "sessionID": "ses_65b3acf58ffe...",
    "type": "text",
    "text": "Here's what I found...",
    "time": { "start": 1759406015783, "end": 1759406015783 }
  }
}
```

### Known event types

| Top-level `type` | `part.type` | Maps to |
|-------------------|-------------|---------|
| `"step_start"` | `"step-start"` | Skip (session init) |
| `"text"` | `"text"` | `StreamEvent::Text(part.text)` |
| `"step_finish"` | `"step-finish"` | `StreamEvent::Result` with cost/tokens |

### step_finish part

```json
{
  "type": "step-finish",
  "tokens": {
    "input": 1234,
    "output": 567,
    "reasoning": 0,
    "cache_read": 800,
    "cache_write": 200
  },
  "cost": 0.0523
}
```

### Tool events (likely)

Tool call events probably exist but aren't documented in the NDJSON schema. If they follow the pattern, they'd be:

```json
{
  "type": "tool_call",
  "part": { "type": "tool-call", "name": "edit", "input": {...} }
}
```

Handle gracefully: if we see unknown types, `Passthrough` them for now. Add specific parsing when we can capture real output.

## Build

### Disambiguation

OpenCode events use `"text"`, `"step_start"`, `"step_finish"` as top-level types. These don't conflict with existing agents:
- Claude uses `"assistant"`, `"system"`, `"user"`
- Codex uses `"item.started"`, `"turn.completed"`
- Gemini uses `"message"`, `"tool_use"`

The `"text"` type is unique to opencode. `"result"` is shared with Claude/Gemini but opencode uses `"step_finish"` instead. No conflicts.

To be safe, disambiguate by checking for the `"sessionID"` field (opencode-specific) or the `"part"` wrapper.

### Parse functions (`stream.rs`)

```rust
// In feed_line match block — add before the catch-all:

// ── OpenCode ─────────────────────────────────────────────
"text" if v.get("sessionID").is_some() => {
    match parse_opencode_text(&v) {
        Some(event) => ParseResult::Events(vec![event]),
        None => ParseResult::Skipped,
    }
}
"step_start" => ParseResult::Skipped,
"step_finish" => {
    ParseResult::Events(vec![parse_opencode_finish(&v)])
}
```

```rust
fn parse_opencode_text(v: &serde_json::Value) -> Option<StreamEvent> {
    let text = v
        .get("part")
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())?;
    if text.is_empty() {
        return None;
    }
    Some(StreamEvent::Text(text.to_string()))
}

fn parse_opencode_finish(v: &serde_json::Value) -> StreamEvent {
    let part = v.get("part");
    let cost = part
        .and_then(|p| p.get("cost"))
        .and_then(|c| c.as_f64());
    // OpenCode doesn't emit duration — compute from step_start/step_finish timestamps
    // or leave as None for now.
    StreamEvent::Result {
        subtype: ResultSubtype::Success,
        cost_usd: cost,
        duration_secs: None,
    }
}
```

### Tests

```rust
#[test]
fn parse_opencode_text_event() {
    let mut parser = StreamParser::new();
    let line = r#"{"type":"text","timestamp":1759406015783,"sessionID":"ses_abc","part":{"type":"text","text":"Hello world","time":{"start":1759406015783,"end":1759406015783}}}"#;
    match parser.feed_line(line) {
        ParseResult::Events(events) => {
            assert_eq!(events.len(), 1);
            assert_eq!(events[0], StreamEvent::Text("Hello world".to_string()));
        }
        other => panic!("Expected Events, got {:?}", other),
    }
}

#[test]
fn parse_opencode_step_start_skipped() {
    let mut parser = StreamParser::new();
    let line = r#"{"type":"step_start","timestamp":1759406015000,"sessionID":"ses_abc","part":{"type":"step-start"}}"#;
    assert_eq!(parser.feed_line(line), ParseResult::Skipped);
}

#[test]
fn parse_opencode_step_finish() {
    let mut parser = StreamParser::new();
    let line = r#"{"type":"step_finish","timestamp":1759406020000,"sessionID":"ses_abc","part":{"type":"step-finish","tokens":{"input":1234,"output":567},"cost":0.05}}"#;
    match parser.feed_line(line) {
        ParseResult::Events(events) => {
            assert_eq!(events.len(), 1);
            match &events[0] {
                StreamEvent::Result { cost_usd, .. } => {
                    assert_eq!(*cost_usd, Some(0.05));
                }
                other => panic!("Expected Result, got {:?}", other),
            }
        }
        other => panic!("Expected Events, got {:?}", other),
    }
}

#[test]
fn parse_opencode_unknown_passthrough() {
    let mut parser = StreamParser::new();
    let line = r#"{"type":"tool_call","sessionID":"ses_abc","part":{"type":"tool-call","name":"edit"}}"#;
    // Unknown opencode event types pass through
    assert_eq!(parser.feed_line(line), ParseResult::Passthrough);
}
```

## Constraints

- Event types must not conflict with existing parsers — use `sessionID` guard on `"text"`
- Unknown event types → `Passthrough` (raw output to user), not `Skipped`
- Parser stays stateless (unit struct)
- Tool call events: gracefully degrade until we capture real output samples
- Cost is in the `part.cost` field of `step_finish`, not in a separate `"result"` event

## Done when

```bash
cargo test -p loopflow -- opencode_stream
cargo test -p loopflow -- opencode
```
