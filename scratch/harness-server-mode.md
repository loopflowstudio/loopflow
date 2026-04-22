# Session Input API

Wave: `lfd` · Item: `01-session-input.md`

## Problem

A conversation runs on the user's computer. They walk away, open their phone, and want to keep going — read what the agent has been doing, and reply. Today there's no way to do this without a terminal. The phone needs to read the session and send a message back into it.

Tool approvals are explicitly **not** part of this story. All tools auto-approve. The phone is for conversation continuity, not safety gating.

## Approach

One new endpoint. Read uses what's already there.

- **Read: existing SSE event stream.** `GET /v0/sessions/{id}/events` already streams `SessionEvent`s with `after_seq` replay. Mobile renders these as a conversation — `TextDelta`, `ItemStarted/Completed`, `TurnStarted/Completed`. Zero changes.
- **Write: `POST /v0/sessions/{id}/input` with `{"text": "..."}`.** Routes to the harness's existing `send_input(content)`. The Codex harness already picks `turn/steer` when a turn is active and starts a new turn when idle (`harness/codex.rs:138-164`). Just expose it.
- **Codex only in v1.** The Claude harness spawns a fresh subprocess per turn (`harness/claude.rs:114-118`), so mid-turn steering is structurally impossible without switching to the undocumented `stream-json` control protocol. Between-turn input would work, but partial support produces a confusing UX (interrupt button silently no-ops). Defer Claude entirely until we adopt `claude-agent-sdk`.
- **Capability flag on session DTO.** `input_supported: bool`. Codex sessions: `true`. Claude/OpenCode: `false`. UI uses this to enable/disable the input field.
- **Auto-approve stays as-is.** No changes to `harness/codex.rs:277-285`. No approval API, no pending-prompts table, no TTL.

## De-risking

| Question | Finding | Impact |
|----------|---------|--------|
| Is the Codex `send_input` path already in place? | Yes. `harness/codex.rs:138` handles steer-vs-new-turn internally. | `POST /input` is a thin HTTP wrapper. |
| Does the existing SSE stream carry enough to render a conversation on mobile? | Yes — `TextDelta`, `ItemStarted`/`ItemCompleted`, `TurnStarted`/`TurnCompleted`, `ContextSnapshot`, `Error` all already exist (`sessions/types.rs:271-341`). | No new event variants needed for v1. |
| Can the Claude harness do mid-turn input today? | No — one-shot subprocess per turn, stdin null. Between-turn works (it's what `send_input` already does), but mid-turn requires `--input-format stream-json` (undocumented control protocol). | Defer Claude to a follow-up. Don't ship "Claude works between turns only" — silent half-functionality. |
| What about reconnect? | `after_seq` replay already works via SSE; existing functionality. | No work. |
| What if the phone sends input while a turn is running on Codex? | `send_input` returns `TurnAlreadyInProgress` if entered with the guard already held — but the Codex impl actually routes to `turn/steer` and Codex appends the message to the current turn. | Verify in the integration test; expected to work. |
| Concurrent input from desktop + mobile? | Both clients hit the same `send_input` path. The `TurnInProgressGuard` serializes; second caller gets `TurnAlreadyInProgress` if the first hasn't routed yet. | Acceptable for v1 — race is sub-second and the second client can retry. Document. |

## Alternatives considered

| Approach | Why not |
|----------|---------|
| Ship Claude with between-turn input only | Half-working features confuse users more than missing ones. The interrupt button would silently do nothing on the most-common harness. |
| Reverse-engineer Claude's stream-json control protocol now | Undocumented, unstable. Belongs in the `claude-agent-sdk` follow-up wave. |
| Add an `input_mode: "steer" \| "new_turn"` field to the request | Premature. `send_input` already does the right thing internally based on turn state. Single-shape API is cleaner. |
| Skip the capability flag — let clients try and handle the error | Forces UI guessing. One bool is cheap and explicit. |

## Key decisions

- **Auto-approve all tools, full stop.** The phone story is conversation continuity, not safety gating. No approval API now or in this item's follow-ups.
- **Codex-only in v1.** Better to ship one harness fully working than two harnesses with confusing partial support.
- **Reuse `send_input`.** Don't model steer-vs-new-turn at the HTTP layer. The harness already encapsulates it.
- **`input_supported` capability flag.** Single bool on the session DTO. UI uses it to enable/disable the input field.
- **No `QuestionAsked`/`QuestionAnswered` events.** Agents don't routinely ask questions outside approval flows. If they do via text, the existing event stream already carries it; the user replies via `POST /input` like any other message.

## Scope

**In:**
- `POST /v0/sessions/{id}/input` route. Body: `{"text": "..."}`. Routes to `Harness::send_input`.
- `input_supported: bool` field on the session DTO. `true` for Codex sessions, `false` for Claude/OpenCode.
- Integration test: spawn Codex session, observe SSE, send input mid-turn (assert steered into the running turn), send input between turns (assert new turn starts), disconnect SSE 10s and reconnect with `after_seq` (assert no events lost).
- Short README under `rust/loopflow/src/lfd/sessions/` covering the input endpoint and capability flag.

**Out:**
- Approvals — out forever, not deferred. Tools auto-approve.
- Claude harness input — deferred to the `claude-agent-sdk` follow-up.
- OpenCode harness input — deferred.
- `QuestionAsked`/`QuestionAnswered` event variants.
- Pending-prompts table, TTLs, decision enums.
- iPhone Concerto client itself (separate wave).
- WebSocket migration of session events.

## Done when

```bash
cargo test -p loopflow --test session_input_round_trip
```

1. Spawn a Codex-backed session via lfd's HTTP API.
2. Send an initial turn; observe `TextDelta` events on `GET /v0/sessions/{id}/events?after_seq=N`.
3. While the turn is in progress, `POST /v0/sessions/{id}/input` with `{"text":"also check the tests"}` returns 200; assert the steered content appears in the running turn (Codex `turn/steer`).
4. After `TurnCompleted`, `POST /v0/sessions/{id}/input` again; assert a new `TurnStarted` event with the posted text.
5. Disconnect the SSE subscriber for 10s mid-flow, reconnect with `after_seq`, receive every missed event.
6. Spawn a Claude-backed session; assert the session DTO shows `input_supported: false`; `POST /input` returns 4xx with a clear "input not supported for this harness" error.

## Wave alignment

The wave item was renamed from "Harness Server Mode" to "Session Input" to match this scope. Finish line updated to "A second device can read a running agent session and send messages into it — including mid-turn — without terminal access."

The wave's three-plane architecture is unchanged. This is still the structured plane — just narrower than originally scoped. Read uses the existing SSE event stream; write uses one new endpoint. No new transport, no new auth surface. The "structured plane drifts from terminal plane" risk from the wave doc is mitigated by reusing the same `SessionEvent` stream both planes already consume.
