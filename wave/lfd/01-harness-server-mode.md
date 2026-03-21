# 01: Harness Server Mode

**Finish line:** Non-terminal clients (iPhone Concerto) can interact with agent sessions through a structured API — observing output, responding to tool approvals and questions — without terminal access.

## Context

The three-plane architecture splits terminal I/O from structured interaction. The terminal plane connection contract is shipped: `lfd` returns `TerminalConnectionInfo` and Concerto builds tmux/SSH commands. The structured plane is the next capability.

The agent harness (Claude Code, Codex, etc.) runs in a mode where `lfd` mediates interactions:

- Agent requests tool approval → `lfd` forwards to client, collects response
- Agent asks a question → `lfd` forwards to client, collects answer
- Agent produces output → `lfd` streams structured output to client
- Client sends input → `lfd` routes to agent

This is the harness's own interaction protocol, not a terminal protocol. The API surface is tool calls, questions, approvals, structured output — not terminal bytes rendered in a web view.

## Auth

OAuth / tokens. Same auth model as the existing WebSocket connection and `lfd` API endpoints. SSH auth is irrelevant here — structured clients don't need terminal access.

## Alternatives rejected

- **Terminal I/O for mobile clients.** Wrong abstraction. A 4-inch screen showing raw terminal output is bad UX regardless of transport quality.
- **WebSocket byte bridge.** Lossy, reinvents terminal transport poorly.

## Open questions

- Exact API shape and harness integration details need a design pass.
- Which harnesses support server/non-interactive mode today?
- How to handle long-running sessions where the mobile client disconnects and reconnects?
