---
status: todo
phase: 2
---
# OpenCode Stream Parser

Parse opencode's NDJSON streaming output into `StreamEvent` types.

## Problem

`StreamParser::feed_line` handles Claude, Codex, and Gemini output but not OpenCode. When `lf implement --agent opencode` runs with streaming, OpenCode's NDJSON lines fall through to `Passthrough` and dump raw JSON to the user. OpenCode needs the same formatted streaming experience as the other three agents.

## Approach

Add OpenCode event parsing directly into the existing `feed_line` match block in `stream.rs`. Three new match arms, two helper functions. Stateless—no changes to the parser struct.

OpenCode's NDJSON format (`opencode run --format json`) emits one JSON object per line:

```json
{"type":"text","timestamp":1759406015783,"sessionID":"ses_abc","part":{"type":"text","text":"Hello world"}}
{"type":"step_start","timestamp":1759406015000,"sessionID":"ses_abc","part":{"type":"step-start"}}
{"type":"step_finish","timestamp":1759406020000,"sessionID":"ses_abc","part":{"type":"step-finish","tokens":{"input":1234,"output":567},"cost":0.05}}
```

### Match arms

Add before the shared `"result"` arm:

```rust
// ── OpenCode ─────────────────────────────────────────────
// OpenCode emits text/step_start/step_finish with a "part" wrapper.
// Guard "text" on sessionID to avoid conflicts with future agents.
"text" if v.get("sessionID").is_some() => {
    match parse_opencode_text(&v) {
        Some(event) => ParseResult::Events(vec![event]),
        None => ParseResult::Skipped,
    }
}
"step_start" => ParseResult::Skipped,
"step_finish" => ParseResult::Events(vec![parse_opencode_finish(&v)]),
```

### Helpers

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
    StreamEvent::Result {
        subtype: ResultSubtype::Success,
        cost_usd: cost,
        duration_secs: None,
    }
}
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Pre-detect agent format on first event, then dispatch | More robust disambiguation but requires stateful parser | Parser is a unit struct by design. Statefulness adds complexity for zero benefit with four agents. |
| Use `part` field instead of `sessionID` for guard | Slightly simpler guard expression | `sessionID` is definitively opencode-specific. `part` is a generic JSON pattern any future agent could use. |
| Parse tool events speculatively | Would handle `tool_call` events if they exist | No real samples of tool event NDJSON. Speculative parsing risks wrong assumptions. Unknown types already `Passthrough` to the user. |

## Key decisions

- **`sessionID` guard on `"text"` only.** `"step_start"` and `"step_finish"` are unambiguous—no other agent uses them. `"text"` needs the guard because it's generic enough that another agent format could use it.
- **No duration.** OpenCode doesn't emit duration in its step_finish event. We could compute it from `step_start`/`step_finish` timestamps, but that requires state. Leave as `None`—cost is the more useful metric anyway.
- **Unknown types → Passthrough.** Tool events (`tool_call`, etc.) aren't documented in OpenCode's NDJSON schema. Unknown types fall through the catch-all `_` arm to `Passthrough`, which prints raw JSON. Users still see everything; we add structured parsing when we capture real samples.
- **Cost in `part.cost`.** Unlike Claude/Gemini which use a separate `"result"` event, OpenCode puts cost in the `step_finish` part. No conflict with the shared `"result"` arm because OpenCode uses `"step_finish"` instead.

## Scope

- In scope: parsing `text`, `step_start`, `step_finish` events; tests for all three plus unknown event passthrough
- Out of scope: tool event parsing (no real samples), duration computation (requires state), token count extraction (no current `StreamEvent` field for it)

## Done when

```bash
cargo test -p loopflow -- opencode
# Specifically:
# - parse_opencode_text_event
# - parse_opencode_step_start_skipped
# - parse_opencode_step_finish
# - parse_opencode_unknown_passthrough
# - parse_opencode_empty_text_skipped (empty text → Skipped, not Events)
cargo fmt --check
cargo clippy -- -D warnings
```
