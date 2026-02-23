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

