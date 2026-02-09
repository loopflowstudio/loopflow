# OpenCode Stream Parser — Review

## What was implemented

Added OpenCode NDJSON stream parsing to `StreamParser::feed_line` in `stream.rs`. Three new match arms handle `text`, `step_start`, and `step_finish` events. Two helper functions (`parse_opencode_text`, `parse_opencode_finish`) extract text content and cost from OpenCode's `part`-wrapped JSON format. Five tests cover all event types including edge cases.

## Key choices

- **`sessionID` guard on `"text"` only.** `step_start` and `step_finish` are unambiguous — no other agent uses them. `"text"` gets a guard because it's generic enough for future conflicts. `sessionID` is definitively opencode-specific.
- **Stateless parser.** No struct changes. Cost comes from `step_finish.part.cost`; duration is `None` because computing it from timestamp pairs would require state.
- **Unknown types → Passthrough.** Tool events (`tool_call`, etc.) aren't in OpenCode's documented NDJSON schema. They fall through the catch-all `_` arm and print raw JSON. Users see everything; structured parsing comes when we capture real samples.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Pre-detect agent format on first event, then dispatch | More robust disambiguation but requires stateful parser | Parser is a unit struct by design. Statefulness adds complexity for zero benefit with four agents. |
| Use `part` field instead of `sessionID` for guard | Slightly simpler guard expression | `sessionID` is definitively opencode-specific. `part` is a generic JSON pattern any future agent could use. |
| Parse tool events speculatively | Would handle `tool_call` events if they exist | No real samples of tool event NDJSON. Speculative parsing risks wrong assumptions. Unknown types already `Passthrough` to the user. |

## How it fits together

`feed_line` dispatches by the `"type"` field of each JSON line. OpenCode's three match arms sit between the Gemini section and the shared `"result"` arm. The helpers follow the same pattern as `parse_gemini_message` and `parse_gemini_tool_use` — extract from JSON, return `Option<StreamEvent>` or `StreamEvent`.

## Risks

- **Tool events are unstructured.** If OpenCode emits `tool_call` events during real usage, they'll show as raw JSON via `Passthrough`. Intentional degradation — no data loss, just less formatting. Fix when we capture real tool event samples.
- **`step_start`/`step_finish` naming.** If a future agent uses these same type strings, they'd be caught by these arms. Low risk — the `sessionID` guard pattern is available if needed.

## Not included

- Tool event parsing (no real samples to work from)
- Duration computation (would require stateful parser)
- Token count extraction (no `StreamEvent` field for it)
- README update for opencode as a supported agent (planned for PR 04)
