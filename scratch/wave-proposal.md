status: proposed

# Agent API: Interactive Agents as First-Class Runtime

## Context

You said:

> "Focused on pyramid-style design: lots of depth detail and production quality at the API/lfd level, just a very minimal slice at the Ui level"

> "UI not important to start, just want to get api right"

> "sessions should probably be resumable"

> "non goal for v1 is confirming that opencode actually works and testing it"

We want Concerto interactive steps to run through a durable lfd agent runtime, with a unified HTTP/SSE protocol that can evolve independently of agent TUIs.

## Scope

Build a staged wave under `wave/agentapi` that prioritizes lfd protocol/runtime quality and keeps UI minimal.

## Approach

1. **Stage 1 — Protocol spine + durable agent runtime + fake adapter (ship first)**
   - Add interactive agent domain types, storage, lifecycle manager, and SSE replay/follow API.
   - Model hierarchy as `wave_runs` (parent) → `agents` (child) → `agent_events` (child).
   - Define provider-agnostic event model (`message/tool/status/input_request/raw`).
   - Add deterministic fake adapter for e2e contract tests.
   - Replace current Concerto PTY-first interactive panel with minimal HTTP chat + End button against new API.
   - Keep wave continue semantics explicit and idempotent from agent end.

2. **Stage 2 — Codex adapter (structured-first)**
   - Map Codex app-server protocol events to unified events.
   - Support approvals/user-input mapping into `input_request` + user responses.
   - Contract-test event mapping and lifecycle transitions.

3. **Stage 3 — Claude adapter v1 (PTY translator)**
   - Run real Claude interactive in lfd-owned PTY.
   - Translate PTY stream to unified events with honest capability flags (best-effort structured prompts, guaranteed free-text + raw fallback).
   - Keep Claude OAuth fully CLI-owned.

4. **Stage 4 — Claude SDK URL parity path (optional upgrade)**
   - Continue probing `--sdk-url` transport and handshake semantics.
   - If parity is achieved, swap Claude adapter transport behind same API/capabilities.

5. **Stage 5 — OpenCode adapter (implementation + validation)**
   - Implement OpenCode adapter using unified protocol contract.
   - Validate end-to-end behavior; close v1 non-goal.
