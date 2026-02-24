# Agent API

Unified session API for interactive coding agents. lfd spawns and manages provider processes (Codex, Claude, OpenCode), translates events into a canonical model, persists everything for replay. Concerto and future clients connect via HTTP/SSE.

`lf` is unchanged. This is purely a new lfd API surface.

## North Star

lfd exposes a provider-agnostic session API. Clients create sessions, send input, subscribe to events, and end sessions. lfd owns the agent lifecycle — spawning processes, translating events, persisting state. Clients connect and disconnect freely; sessions survive reconnect.

## Design Decisions

**Harness-first, not protocol-first.** Build a working harness end-to-end before abstracting. The protocol emerged from real provider behavior — Codex first, then Claude validated the abstraction. Harness mapping modules (`codex_mapping.rs`, `claude_mapping.rs`) now live alongside harness logic.

**lfd owns the session lifecycle.** Concerto is a thin client. Agent processes survive Concerto close/reopen. Session state lives in lfd's store.

**Turn+item event model.** Provider harnesses translate native events (Codex JSON-RPC, Claude NDJSON, OpenCode SSE) into a canonical event model. Turns are first-class (every turn-scoped event carries `turn_id`). Items are typed (`Command`, `File`, `Message`, `Thought`, `Tool`) with lifecycles (`ItemStarted → ItemUpdated → ItemCompleted`). High-frequency deltas (`TextDelta`, `ReasoningDelta`) stay top-level for streaming efficiency.

**Container-first safety.** All harnesses run in yolo/bypass mode. Safety comes from the container, not tool-level permissions. No approval routing in v1.

**Provider-owned auth.** Codex and Claude handle their own OAuth/login. lfd never collects raw credentials.

## Invariants

- At most one active session per wave run at a time
- Session end is idempotent (multiple calls safe)
- UI disconnect does not affect session state
- Reconnect replays persisted events then follows live stream
- Session end triggers wave continue only when wave run still waits on that session

## Phases

| # | Phase | What it unlocks | Status |
|---|-------|----------------|--------|
| 01 | Unified Session API + Codex | Session API, event model, storage, SSE replay. Codex as first harness. | shipped |
| 02 | Claude Harness | `-p --resume` with structured output. Probes agent personality. | shipped |
| 03 | Concerto UI | Typed transcript, item cards, session lifecycle, reconnect/replay | shipped |
| 04 | Hardening | Reconnect, concurrent clients, crash recovery, wave integration | |
| 05 | Claude `--sdk-url` | Reference only — not pursuing unless landscape changes | |
| 06 | OpenCode Harness | Third provider harness validates the abstraction | |
| 07 | Provider Layer Unification | Make provider harnesses the shared core used by both `lf` CLI runs and `/v0/sessions` HTTP sessions | planned |

## Future direction

After Phase 06, unify the provider layer so `lf` and Session HTTP are explicitly two API surfaces over the same harness core:

- one provider execution/mapping core
- two entry points (`lf` CLI and `lfd` Session HTTP)
- shared conformance tests across both surfaces

## Architecture

```
Concerto ──HTTP/SSE──▶ lfd session API
                         ├── SessionManager (lifecycle, state machine)
                         │     └── SessionRuntime (harness + broadcast + seq counter)
                         ├── session store (sessions + session_events tables)
                         └── provider harness (Codex | Claude | OpenCode)
                               ├── event bridge task (harness → store + broadcast)
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
