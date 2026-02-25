# Agent API

Unified session API for interactive coding agents. lfd spawns and manages provider processes (Codex, Claude, OpenCode), translates events into a canonical model, persists everything for replay. Concerto and future clients connect via HTTP/SSE.

`lf` headless runs are unchanged. Interactive `lf` commands (`design`, `explore`, `review`, `refine`) will move to the session API + Concerto UI (see `01-runtime-convergence.md`).

## Vision

lfd exposes a provider-agnostic session API. Clients create sessions, send input, subscribe to events, and end sessions. lfd owns the agent lifecycle — spawning processes, translating events, persisting state. Clients connect and disconnect freely; sessions survive reconnect.

### Not here

- Approval routing / permission management (container safety instead)
- Advanced Concerto UI (tool visualization, diff views, multi-panel layouts)
- Multi-agent per step (parallel agents within one interactive step)
- Cross-wave session sharing

## Goals

- Provider-agnostic: same client code works regardless of which agent runs the session
- Harness-first: build working harnesses end-to-end before abstracting (protocol emerged from real provider behavior)
- lfd owns the session lifecycle — Concerto is a thin client, agent processes survive close/reopen
- Turn+item event model with typed items (`Command`, `File`, `Message`, `Thought`, `Tool`) and explicit lifecycles
- Container-first safety — harnesses run in yolo/bypass mode, safety comes from the container
- Provider-owned auth — Codex and Claude handle their own OAuth/login, lfd never collects raw credentials
- At most one active session per wave run at a time
- Session end is idempotent; UI disconnect does not affect session state
- Reconnect replays persisted events then follows live stream

## Risks

- **Provider layer drift.** `lf` CLI and `/v0/sessions` HTTP use separate execution paths through the same harnesses. Changes to one path can miss the other. Mitigate: shared conformance tests once the third harness validates the abstraction.
- **Claude `--resume` fragility.** Each turn spawns a new process with `--resume`. If the resume format changes or session state corrupts, the entire session breaks with no partial recovery. Mitigate: session events are persisted, so replay from a new session is possible even if resume fails.
- **Container-only safety model.** No tool-level permission routing means local (non-container) sessions run with full agent permissions. Acceptable for v1 but becomes a gap if local interactive sessions grow in usage.
- **SSE replay scalability.** Long sessions accumulate events; full replay on reconnect grows linearly. Not a problem at current scale but could become one with multi-hour sessions.

## Metrics

- All three provider harnesses (Codex, Claude, OpenCode) pass shared conformance tests
- Session reconnect replays full event history and resumes live streaming without data loss
- Concerto renders typed transcript with item cards for all event types
- Session lifecycle (create → interact → end) works identically for local and remote lfd

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

## Future direction

After the OpenCode adapter validates the abstraction, unify the provider layer so `lf` and Session HTTP are explicitly two API surfaces over the same harness core.
