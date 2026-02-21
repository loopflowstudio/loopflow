# Agent API

Interactive agents as first-class lfd runtime. Unified HTTP/SSE protocol for Concerto to interact with coding agents (Codex, Claude, OpenCode) during interactive wave steps.

## North Star

Wave reaches an interactive step. lfd launches the agent, owns its lifecycle, and exposes a provider-agnostic event stream. Concerto connects, shows a chat UI, sends input, and disconnects freely — the agent keeps running. User clicks End, lfd commits and advances the wave.

## Design Decisions

**Adapter-first, not protocol-first.** Build a working Codex adapter end-to-end before abstracting. The protocol emerges from real adapter behavior, not upfront design.

**lfd owns the agent lifecycle.** Concerto is a thin client. Agent processes survive Concerto close/reopen. Session state lives in lfd's store.

**Structured events over terminal bytes.** Provider adapters translate native events (Codex JSON-RPC, Claude PTY output) into a canonical event model. Raw fallback for anything the adapter can't parse.

**Honest capability flags.** Each adapter advertises what it can do (structured input requests, tool events, interrupts). UI renders from capabilities, not provider name. Partial support is explicit.

**Provider-owned auth.** Codex and Claude handle their own OAuth/login. lfd never collects raw credentials.

## Invariants

- At most one active interactive agent per wave run at a time
- Agent end is idempotent (multiple calls safe)
- UI disconnect does not affect agent state
- Reconnect replays persisted events then follows live stream
- Agent end triggers wave continue only when wave run still waits on that agent

## Phases

| # | Phase | What it unlocks | Status |
|---|-------|----------------|--------|
| 01 | Codex End-to-End | Working interactive agent: launch, events, input, end, persist, replay | |
| 02 | Claude Adapter | Second adapter proves the abstraction; PTY translator with honest capabilities | |
| 03 | Concerto UI | Minimal chat panel, input, End button against proven API | |
| 04 | Hardening | Reconnect, resume, edge cases, wave continue integration | |
| 05 | Claude SDK URL | Optional transport upgrade if parity achieved | |
| 06 | OpenCode Adapter | Third adapter validates protocol is truly provider-agnostic | |

## Architecture

```
wave_runs (orchestration parent)
  └── agents (execution child, agents.wave_run_id)
        └── agent_events (event history, agent_events.agent_id)
```

```
Concerto ──HTTP/SSE──▶ lfd agent API
                         ├── AgentManager (lifecycle, state machine)
                         ├── agent store (agents + agent_events tables)
                         └── adapter (Codex | Claude | OpenCode | Fake)
                               └── provider process (Codex app-server | Claude PTY | ...)
```

## API (agent-centric)

- `POST /waves/{wave_id}/agents` — create + launch
- `GET /waves/{wave_id}/agents/active` — reconnect entrypoint
- `GET /agents/{agent_id}` — status + metadata
- `POST /agents/{agent_id}/input` — send user input
- `POST /agents/{agent_id}/end` — graceful shutdown + wave continue
- `GET /agents/{agent_id}/events` — SSE replay + follow

## What's not here

- Advanced Concerto UI (tool visualization, diff views, multi-panel layouts)
- Multi-agent per step (parallel agents within one interactive step)
- Cross-wave agent sharing
