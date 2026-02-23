# 05: Reference — Claude `--sdk-url` (not pursuing)

Notes on Claude's `--sdk-url` WebSocket transport. We're not using this approach, but keeping it as reference in case the landscape changes.

## What `--sdk-url` does

Overrides Claude's output transport. Instead of rendering to a terminal, Claude sends events to a WebSocket URL. The flow is inverted — Claude pushes to a server you run, rather than you pulling from Claude.

```bash
claude --sdk-url ws://localhost:8080
```

## Why we're not using it

- **Undocumented.** No official docs on the protocol, event types, or expected behavior.
- **Untested for interactive use.** Early probes suggest it's a headless print-mode transport override, not an interactive session bridge.
- **Unclear OAuth behavior.** May not preserve the OAuth flow that Claude Pro/Max users need.
- **Unclear agent personality.** Likely degrades to non-interactive behavior similar to `-p` mode.
- **Inverted architecture.** lfd would need to run a WebSocket server for Claude to connect to. Every other adapter has lfd as the client, not the server.

## What would change our mind

- Anthropic documents `--sdk-url` for interactive use with a stable protocol spec
- Someone demonstrates it preserves conversational behavior + OAuth
- The WebSocket protocol carries structured events equivalent to `-p --output-format stream-json`
- Bidirectional communication works (input flows back over the same WebSocket)

## If we pursued it

1. lfd runs a WebSocket server on localhost
2. Spawn `claude --sdk-url ws://localhost:$PORT`
3. Claude sends structured events over WebSocket
4. lfd receives events directly — no process-per-turn overhead
5. Input flows back over the same WebSocket (bidirectional)

This would be the cleanest Claude integration: structured, bidirectional, single long-lived process. But it's speculative until the protocol is documented and proven.

## Probe matrix (if revisiting)

1. **Transport shape**: point `--sdk-url` at a capture server, detect protocol framing
2. **Interactive semantics**: compare event traces between TUI and `--sdk-url` for same prompt
3. **Input fidelity**: verify typed request objects for tool calls and prompts
4. **Session continuity**: disconnect/reconnect, validate resume behavior
5. **Auth ownership**: confirm OAuth stays in Claude process
