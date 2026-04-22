---
notion_id: 333f8f99-3d81-8124-82cb-d3b989884d36
---
# 01: Session Input

**Finish line:** A second device can read a running agent session and send messages into it — including mid-turn — without terminal access.

## Context

The three-plane architecture splits terminal I/O from structured interaction. The terminal plane connection contract is shipped: `lfd` returns `TerminalConnectionInfo` and Concerto builds tmux/SSH commands. The structured plane is the next capability.

The product story is conversation continuity. A user starts a conversation on their computer, walks away, opens their phone, and keeps going — reading what the agent has been doing and replying to it. Tools auto-approve in the background; tool gating is explicitly not part of this item.

Read uses the existing per-session SSE event stream (`SessionEvent`). Write is one new endpoint that routes through the harness's existing `send_input`, which already handles "steer the running turn" vs "start a new turn" internally.

## Scope

- `POST /v1/sessions/{id}/input` — body `{"text": "..."}`, routes to `Harness::send_input`.
- Capability flag `input_supported: bool` on the session DTO.
- Codex sessions only in v1. The Codex harness already handles steer-vs-new-turn.
- Claude and OpenCode return `input_supported: false`. Claude support is deferred to the `claude-agent-sdk` follow-up — partial mid-turn support requires the undocumented stream-json control protocol, which we're choosing not to reverse-engineer.

## Auth

OAuth / tokens. Same auth model as the existing WebSocket connection and `lfd` API endpoints. Bearer token middleware on `/v1/sessions/*` already covers this.

## Alternatives rejected

- **Terminal I/O for mobile clients.** Wrong abstraction. A 4-inch screen showing raw terminal output is bad UX regardless of transport quality.
- **Approvals API in v1.** Auto-approve is the explicit product policy. Adding an approval channel would be scope and a half-functional UX (Claude can't speak the protocol).
- **Ship Claude with between-turn input only.** Half-working features confuse users more than missing ones. The interrupt button would silently do nothing on the most-used harness. Defer to the SDK follow-up so Claude lands fully functional.

## Design

Detailed approach in `scratch/harness-server-mode.md`.
