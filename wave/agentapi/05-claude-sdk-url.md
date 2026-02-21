# 05: Claude SDK URL Upgrade

Optional transport swap for Claude adapter — replace PTY with SDK URL WebSocket if parity is achieved.

## What exists after this

If Claude's `--sdk-url` WebSocket transport reaches behavioral parity with interactive TUI, the Claude adapter uses it instead of PTY. Same API, same capabilities (or better), no terminal scraping. If parity isn't achieved, this phase is skipped — PTY adapter continues working.

## Probe matrix (gate before implementation)

1. **Transport shape**: Point `--sdk-url` at capture server, detect protocol framing
2. **Interactive semantics**: Compare event traces between TUI and SDK URL for same prompt
3. **Input-request fidelity**: Verify typed request objects for approvals and option prompts
4. **Session continuity**: Disconnect/reconnect, validate resume behavior
5. **Auth ownership**: Confirm OAuth stays in Claude process

## Decision gate

- Probes 1-4 pass with acceptable parity → swap transport, keep same API
- Transport works but choice/approval fidelity weak → gated beta with explicit limitations
- Transport fails parity → skip this phase, PTY adapter remains

## Done when

- Probe results documented with pass/fail per item
- If passing: Claude adapter uses SDK URL transport, all existing tests pass
- If failing: decision documented, PTY adapter confirmed as long-term path
