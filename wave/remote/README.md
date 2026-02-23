# Remote

Connect Concerto to a remote lfd. Same HTTP+WS protocol, different host.

## Vision

lfd runs on a remote Linux machine (containerized). Concerto connects from your Mac (or phone). Waves run remotely with full agent execution. You open Cursor via Remote SSH when you need to edit files. One-click per wave.

### Scope boundaries (not here)

| # | Phase | What it unlocks | Pre-work | Status |
|---|-------|----------------|----------|--------|
| 01 | Sandboxed Agents | Agents in Docker containers, controlled credentials, fork parity | None | Shipped |
| 02 | Compose Stack | Full stack in Docker (lfd + postgres), test locally | 01 | Shipped |
| 03 | Pre-shared Token Auth | lfd accepts remote connections | None | Shipped |
| 04 | EC2 Infrastructure | A box to deploy on (Docker + compose) | 02 | Shipped |
| 05 | Concerto Remote Connection | Wave CRUD, events, logs over WAN | 03, 04 | Next |
| 06 | Remote File Access | One-click "Open in Cursor" per wave | 05 |
| 07 | Studio Auth | Real JWT auth via auth.loopflow.studio (server built, client wiring remaining) | 05 |
| 08 | API Expansion | File browsing, step/flow/direction typeahead | 05 |
| 09 | Hosted SaaS | loopflow.studio runs lfd for you | 07, 08 |

## Goals

- Keep protocol parity: Concerto talks to local and remote lfd via the same HTTP/WS surface
- Keep orchestration ownership in loopflow while editors handle their native remote transport
- Ship remote connectivity incrementally: secure auth first, then UX and API breadth

## Update after Phase 01E (Docker fork parity)

Phases 01A–01E are fully shipped. What we now treat as non-negotiable for remote work:

- Fork semantics are the same in CLI, daemon, and Docker executor: run all branches, then synthesize.
- Docker forks use sibling worktrees in container volumes (`/workspace/repos/<repo>/worktrees/<wave>-fork-N`).
- Scheduler slot release and orphan-fork cleanup must be restart-safe (covers both native and Docker).
- Agent timeout is explicit operator config (`executor.agent_timeout`), not hidden watchdog behavior.
- Executor trait abstraction (`AgentExecutor`) handles workspace file ops — no Docker-specific downcasts in fork orchestration.

### Known limitation: host-side git worktrees for prompt assembly

Docker fork branches still rely on host-side git worktrees for prompt assembly (`build_step_prompt` needs local step/context files before container launch). This is acceptable — prompt assembly happens before container launch, and the worktrees are cleaned up after the fork completes. Moving to host placeholder dirs would require either pre-prompt container→host sync or a prompt build path that doesn't depend on host worktree materialization.

### Impact on the next phase (05)

Phase 05 needs to include two correctness checks that were previously implicit:

1. Timeout/fail-fast errors from daemon execution need clear surfacing in Concerto (not generic request failures).
2. Chat events use SSE (`GET /waves/:id/chat/events`), not WebSocket. Phase 05 must verify SSE works through the TLS proxy (Caddy) alongside the existing WebSocket path for run events.

Fork flows (wave-reduce, wave-polish, wave-expand) now work in Docker. Phase 05 does not need to gate or warn about fork capability — remote users can run any flow.

### Open questions for 05/06

- Do we expose `executor.agent_timeout` in Concerto connection settings, or keep it daemon-config only in v1?
- Should remote capability warnings live in wave detail only, or also in wave edit/config surfaces?

### What might change next

If Docker build-context cost is painful on large repos during Phase 05 dogfooding, we may need to pull that optimization earlier than currently planned.

## Metrics

- Remote Concerto sessions can perform wave CRUD, event streaming, and log access over WAN
- SSE chat events and WebSocket run events both succeed through TLS proxy paths
- Remote workflows preserve local behavior expectations (same protocol semantics, auth-gated access)
- One-click wave-to-editor workflow works via Remote SSH once file access phase ships

