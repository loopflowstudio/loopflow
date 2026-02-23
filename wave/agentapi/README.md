# Agent API

Unified session API for interactive coding agents. lfd spawns and manages provider processes (Codex, Claude, OpenCode), translates events into a canonical model, persists everything for replay. Concerto and future clients connect via HTTP/SSE.

`lf` is unchanged. This is purely a new lfd API surface.

## Vision

lfd exposes a provider-agnostic session API. Clients create sessions, send input, subscribe to events, and end sessions. lfd owns the agent lifecycle — spawning processes, translating events, persisting state. Clients connect and disconnect freely; sessions survive reconnect.

### Scope boundaries (not here)

- Advanced Concerto UI (tool visualization, diff views, multi-panel layouts)
- Multi-agent per step (parallel agents within one interactive step)
- Cross-wave agent sharing

## Goals

Concrete objectives and invariants for this wave:

- Build a provider-agnostic interactive runtime that works end-to-end with real adapters
- Keep lifecycle ownership in lfd so clients can disconnect/reconnect without losing state
- Keep adapter capabilities explicit so UI behavior follows capabilities, not provider names

### Invariants

- At most one active interactive agent per wave run at a time
- Agent end is idempotent (multiple calls safe)
- UI disconnect does not affect agent state
- Reconnect replays persisted events then follows live stream
- Agent end triggers wave continue only when wave run still waits on that agent

## Risks

- Canonical event shape may drift as adapters expand beyond Codex/Claude/OpenCode assumptions
- PTY/stream translation gaps can force raw fallbacks and reduce UI fidelity
- Reconnect and persistence correctness can regress as more lifecycle edge cases are added

## Metrics

- End-to-end interactive flow works for each shipped adapter: launch → events → input → end
- Reconnect replay is observable: persisted events appear first, then live follow continues
- Wave advancement only occurs after valid agent end for the waiting wave run

## Roadmap

| # | Phase | What it unlocks | Status |
|---|-------|----------------|--------|
| 01 | Unified Session API + Codex | Session API, event model, storage, SSE replay. Codex as first adapter. | shipped |
| 02 | Claude Adapter | `-p --resume` with structured output. Probes agent personality. | |
| 03 | Concerto UI | Minimal chat panel, input, End button against proven API | |
| 04 | Hardening | Reconnect, concurrent clients, crash recovery, wave integration | |
| 05 | Claude `--sdk-url` | Reference only — not pursuing unless landscape changes | |
| 06 | OpenCode Adapter | Third adapter validates the abstraction | |

## Design Decisions

**Adapter-first, not protocol-first.** Build a working Codex adapter end-to-end before abstracting. The protocol emerges from real adapter behavior, not upfront design.

**lfd owns the session lifecycle.** Concerto is a thin client. Agent processes survive Concerto close/reopen. Session state lives in lfd's store.

**Turn+item event model.** Provider adapters translate native events (Codex JSON-RPC, Claude NDJSON, OpenCode SSE) into a canonical event model. Turns are first-class (every turn-scoped event carries `turn_id`). Items are typed (`Command`, `FileChange`, `McpToolCall`, `AgentMessage`, `Plan`, `Tool`) with lifecycles (`ItemStarted → ItemUpdated → ItemCompleted`). High-frequency deltas (`TextDelta`, `ReasoningDelta`) stay top-level for streaming efficiency.

**Container-first safety.** All adapters run in yolo/bypass mode. Safety comes from the container, not tool-level permissions. No approval routing in v1.

**Provider-owned auth.** Codex and Claude handle their own OAuth/login. lfd never collects raw credentials.

## Architecture

```
Concerto ──HTTP/SSE──▶ lfd session API
                         ├── SessionManager (lifecycle, state machine)
                         │     └── SessionRuntime (adapter + broadcast + seq counter)
                         ├── session store (sessions + session_events tables)
                         └── adapter (Codex | Claude | OpenCode)
                               ├── event bridge task (adapter → store + broadcast)
                               └── provider process
                                     Codex: codex --app-server (JSON-RPC stdio)
                                     Claude: claude -p --resume (NDJSON stdio)
                                     OpenCode: opencode serve (HTTP + SSE)
```

## API

```
POST   /v0/sessions              # create session (provider, config)
GET    /v0/sessions/{id}         # session status + metadata
POST   /v0/sessions/{id}/input   # send user message
GET    /v0/sessions/{id}/events  # SSE replay + follow
DELETE /v0/sessions/{id}         # end session
```

## Provider Comparison

| Provider | Process model | Output | Input | Auth |
|----------|--------------|--------|-------|------|
| Codex | subprocess stdio | JSON-RPC notifications | JSON-RPC requests | OAuth (provider-owned) |
| Claude | subprocess stdio | NDJSON (stream-json) | New process per turn (`--resume`) | OAuth (provider-owned) |
| OpenCode | HTTP client | SSE events | REST calls | Provider-owned |

## What's not here

- Approval routing / permission management (container safety instead)
- Advanced Concerto UI (tool visualization, diff views, multi-panel layouts)
- Multi-agent per step (parallel agents within one interactive step)
- Cross-wave session sharing
