# Remote

Connect Concerto to a remote lfd. Same HTTP+WS protocol, different host.

## Vision

lfd runs on a remote Linux machine (containerized). Concerto connects from your Mac (or phone). Waves run remotely with full agent execution. You open Cursor via Remote SSH when you need to edit files. One-click per wave.

### Scope boundaries (not here)

- Mobile UI — see [mobile/](../mobile/)
- Wave area summaries — separate project
- UX polish (optimistic updates, etc.) — see [ux/](../ux/)

## Goals

- Keep protocol parity: Concerto talks to local and remote lfd via the same HTTP/WS surface
- Keep orchestration ownership in loopflow while editors handle their native remote transport
- Ship remote connectivity incrementally: secure auth first, then UX and API breadth

## Risks

### Update after Phase 01D hardening

Phase 01D shipped and changed what we now treat as non-negotiable for remote work:

- Fork semantics are the same in CLI and daemon: run all branches, then synthesize.
- Scheduler slot release and orphan-fork cleanup must be restart-safe.
- Agent timeout is explicit operator config (`executor.agent_timeout`), not hidden watchdog behavior.
- Fork execution is unsupported in the Docker executor — no worktree-based parallelism in containers. Remote users on Docker are limited to non-fork flows. This is an accepted gap, not a blocker.
- Track dedicated follow-up in `wave/remote/01e-docker-fork-parity.md`.

### Impact on the next phase (05)

Phase 05 now needs to include two correctness checks that were previously implicit:

1. Timeout/fail-fast errors from daemon execution need clear surfacing in Concerto (not generic request failures).
2. Chat events use SSE (`GET /waves/:id/chat/events`), not WebSocket. Phase 05 must verify SSE works through the TLS proxy (Caddy) alongside the existing WebSocket path for run events.

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

