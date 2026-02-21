# 06: OpenCode Adapter

Third adapter validates the protocol is truly provider-agnostic.

## What exists after this

OpenCode server mode works through the same agent API. Three adapters with three different transports (JSON-RPC stdio, PTY, HTTP+SSE) all map cleanly to the canonical event model. Protocol is proven provider-agnostic.

## What to build

- OpenCode adapter using its HTTP API + SSE event stream
- Map OpenCode primitives: session/message resources, permission responses, question prompts
- Validate end-to-end: launch → events → input → end → wave advance

## Done when

- OpenCode interactive sessions work through the agent API
- No protocol changes required to support the third adapter
- All three adapters pass the same contract test suite
