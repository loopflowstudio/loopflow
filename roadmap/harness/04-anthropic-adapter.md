# 04: Anthropic Model Adapter

Implement the first real model provider for `lf-agent`.

## What exists after this

- Anthropic Messages API integration via `reqwest`
- conversion between internal message/tool types and Anthropic payloads
- usage capture (input/output token counts)

## Commit slices

### C1 — Request/response mapping (~300-500 LOC)

- serde request/response structs
- conversion helpers for tools/messages/system
- tool-use extraction into `ToolCallRequest`

### C2 — Error handling + provider tests (~250-450 LOC)

- handle API errors and malformed tool payloads
- fixture-driven parser tests
- usage accounting tests

## Constraints

- No SDK dependency; raw HTTP + serde.
- Keep adapter logic isolated in `model/anthropic.rs`.
- Preserve provider-agnostic `Model` trait.

## Done when

```bash
cargo test -p lf-agent anthropic
```

Expected: adapter parser/request tests pass.
