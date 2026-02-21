# 01: Codex End-to-End

Launch Codex app-server from lfd, stream structured events through a minimal agent API, persist event history, and prove the protocol with real interactive sessions.

## What exists after this

`lf design` (or any interactive step) launches a Codex agent via lfd. Events stream through an SSE endpoint. User input flows back. Ending the agent commits and advances the wave. Event history persists and replays on reconnect. The protocol, storage, and API all emerge from making this one adapter work.

## Why Codex first

Codex app-server has the lowest impedance — it already emits structured JSON-RPC events for turns, items, approvals, and user-input requests. Building against it means the protocol design is informed by real structured events, not guesswork. Claude's PTY adapter requires parsing terminal output; that's harder and should prove the abstraction, not define it.

## What to build

### Codex adapter

- Spawn Codex in app-server mode (`codex --app-server` or equivalent stdio JSON-RPC)
- Map Codex events to canonical agent events:
  - `turn/started`, `turn/completed` → `session_status`
  - `item/text.delta` → `message_delta`
  - `item/text.done` → `message_final`
  - `item/tool_call.*` → `tool_started`, `tool_delta`, `tool_finished`
  - `tool/requestUserInput` → `input_requested` (free text)
  - approval requests → `input_requested` (single choice: approve/deny)
- Forward user input back to Codex via JSON-RPC responses
- Handle graceful shutdown on end

### Agent lifecycle manager

- State machine: `starting → running → waiting_for_user → ending → ended | failed`
- Spawn adapter, wire event sink, track status transitions
- Idempotent end (multiple calls safe)
- At most one active agent per wave run

### Storage

- `agent_events` table — persist every event with sequence number for replay
- Extend existing `agents` table with interactive fields: `provider`, `provider_session_id`, `status`, capability flags
- Align event model with harness `AgentEvent` patterns (message, tool_call, tool_result, done, failed)

### HTTP API

- `POST /waves/{wave_id}/agents` — create agent, launch adapter
- `GET /agents/{agent_id}` — current status + capabilities
- `GET /agents/{agent_id}/events` — SSE replay (from seq 0) + follow (live tail)
- `POST /agents/{agent_id}/input` — free text or option selection
- `POST /agents/{agent_id}/end` — graceful stop, wave continue

### Wave integration

- `WaitInteractive` triggers agent creation + adapter launch
- Agent end executes existing continue/commit logic
- Wave run state guards: end only works when wave run still waits on this agent

## What we'll learn

- Whether the Codex app-server event taxonomy maps cleanly to our canonical model
- What the right granularity is for `agent_events` (every delta? batched? final only?)
- How approval flows and user-input requests actually behave in practice
- Whether SSE replay + follow is sufficient or if we need cursor-based pagination

## Open questions

- Does Codex app-server support session resume after process restart?
- What happens to in-flight turns when we send end?
- How do Codex approval timeouts interact with our lifecycle?

## Done when

- `lf design` with Codex launches an interactive agent via lfd
- Codex events stream through `GET /agents/{id}/events` as SSE
- User input via `POST /agents/{id}/input` reaches Codex and produces a response turn
- `POST /agents/{id}/end` stops the agent and advances the wave
- Event history persists in `agent_events` and replays correctly on new SSE connection
- Integration test covers: launch → input → events → end → wave advance
