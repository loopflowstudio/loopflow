# 02: Claude Adapter

Second adapter proves the protocol abstraction. PTY translator with honest capability flags — structured events where parseable, raw fallback everywhere else.

## What exists after this

Claude interactive sessions work through the same agent API as Codex. lfd spawns Claude in a PTY, translates terminal output into canonical events, and advertises partial capabilities honestly. The protocol survives a second adapter without breaking changes.

## Why PTY

Claude's `--sdk-url` WebSocket transport is undocumented and untested for interactive use. Probe findings (see design doc) show it's a headless print-mode transport override, not an interactive session bridge. PTY preserves real interactive Claude behavior — tool use, permission prompts, multi-turn conversation — without degrading to one-shot semantics.

## What to build

### PTY adapter

- Spawn `claude` in lfd-owned PTY (no `--sdk-url`, no `-p`)
- Stream PTY output through a tiered parser:
  - **Tier A**: Explicit structured markers (if Claude emits any) → direct mapping
  - **Tier B**: Deterministic rules (numbered option lists → `single_choice`, permission prompts → approval)
  - **Tier C**: Raw fallback → `message_delta` with unparsed text
- Write user input directly to PTY stdin
- Handle graceful shutdown (send exit/interrupt, wait for process)

### Capability advertisement

```json
{
  "structured_input_requests": "partial",
  "option_prompts": "partial",
  "free_text_input": true,
  "tool_events": "partial",
  "interrupt": true,
  "raw_output_fallback": true
}
```

Partial means: best-effort detection, never blocks the user, false negatives acceptable.

### Protocol validation

- Same API endpoints, same SSE event stream, same input/end flow
- If the protocol needs changes to support Claude, those changes must also work for Codex
- Any Claude-specific behavior goes in adapter internals, not the API surface

## What we'll learn

- Whether the canonical event model survives a fundamentally different adapter (structured JSON-RPC vs terminal scraping)
- How reliable terminal pattern matching is for input-request detection
- Whether capability flags are sufficient or if we need richer negotiation
- What raw_output_fallback UX actually feels like in practice

## Open questions

- Can we detect Claude's `--resume` session ID from PTY output for recovery?
- How do Claude Code hooks interact with lfd-owned PTY?
- Is ANSI stripping sufficient or do we need a full terminal emulator state machine?

## Done when

- `lf design` with Claude launches an interactive agent via lfd PTY
- Assistant messages stream through SSE as `message_delta` / `message_final`
- Free-text input reaches Claude via PTY stdin
- Permission prompts detected as `input_requested` (at least some of the time)
- End stops Claude process and advances wave
- Codex adapter still works unchanged (no protocol regression)
