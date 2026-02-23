# 02: Claude Adapter

Second adapter using `-p --resume` with structured output. Probes whether agent personality holds in headless mode with system prompt tuning.

## What exists after this

Claude interactive sessions work through the same session API as Codex. lfd spawns Claude with `-p --resume --output-format stream-json`, translates NDJSON events into canonical SessionEvents, and persists everything. The session API survives a second adapter without breaking changes.

## Approach: `-p --resume` with structured output

Use the real `claude` binary (OAuth-compatible). One process per turn.

**Why not PTY:** Structured output is cleaner — no ANSI parsing, no heuristic prompt detection. The trade-off is `-p` mode changes agent personality (more task-completion oriented, less conversational). We mitigate with `--append-system-prompt` and evaluate.

**Why not Agent SDK:** The `claude-agent-sdk` Python package requires API keys. We need OAuth support (Claude Pro/Max plans), which requires the real `claude` binary.

**Why not `--sdk-url`:** See phase 06 reference notes. Undocumented, unclear if it preserves interactive behavior or OAuth.

## What to build

### Claude adapter

**First turn:**
```bash
claude -p "prompt" \
  --output-format stream-json \
  --dangerously-skip-permissions \
  --append-system-prompt "Be conversational. Ask clarifying questions. Show your work." \
  --verbose \
  --include-partial-messages
```

Capture `session_id` from output.

**Subsequent turns:**
```bash
claude -p "follow-up message" \
  --resume $session_id \
  --output-format stream-json \
  --dangerously-skip-permissions \
  --verbose \
  --include-partial-messages
```

**Event mapping:**
- `content_block_delta` (text_delta) → `TextDelta`
- `content_block_start` (tool_use) → `ToolStarted`
- `content_block_delta` (input_json_delta) → `ToolOutput`
- `content_block_stop` → `ToolDone`
- `message_stop` → `TurnCompleted`

### Protocol validation

- Same API endpoints, same SSE event stream, same input/end flow
- If the session API needs changes to support Claude, those changes must also work for Codex
- Any Claude-specific behavior goes in adapter internals, not the API surface

## Probes (gate before committing)

1. **Agent personality**: does `--append-system-prompt` keep Claude conversational in `-p` mode? Run several interactive sessions and evaluate. Is it chatty? Does it ask questions? Does it show progress?
2. **Turn latency**: process-per-turn has startup overhead. Is it acceptable for interactive use?
3. **Session continuity**: does `--resume` preserve full conversation context across many turns?
4. **Tool events**: does `--output-format stream-json` emit structured tool_use blocks that map cleanly?

## Decision gate

- Probes 1-3 pass → ship it, `-p --resume` is the Claude adapter
- Personality is off but tolerable → ship with known limitations, iterate on system prompt
- Personality fundamentally broken → fall back to PTY (phase 06)

## Done when

- `POST /sessions` with `provider: "claude"` spawns Claude process
- Claude events stream through `GET /sessions/{id}/events` as SSE
- `POST /sessions/{id}/input` triggers a new `--resume` turn, Claude responds
- `DELETE /sessions/{id}` stops the agent
- Codex adapter still works unchanged (no API regression)
- Probe results documented
