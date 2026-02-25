# Remote

Connect Concerto to a remote lfd. Same HTTP+WS protocol, different host.

## Vision

lfd runs on a remote Linux machine (containerized). Concerto connects from your Mac (or phone). Waves run remotely with full agent execution. You open Cursor via Remote SSH when you need to edit files. One-click per wave.

## Roadmap

Infrastructure phases (01-04) shipped the foundation. Bundled daemon mode shipped as the default local connection, validating protocol parity end-to-end. The remaining work is deployment-driven: deploy, dogfood, fix what breaks.

| Step | What | Status |
|------|------|--------|
| 0 | Error mapping + remote editor/terminal launch | Shipped |
| — | Bundled daemon runtime + connection mode switcher | Shipped |
| 1 | Deploy lfd to EC2 (Docker + Caddy + static token) | Next |
| 2 | Deploy lfd to Mac Mini (native + launchd), connect Concerto | |
| 3 | Studio auth (Phase 07: JWT validation, sign-in UX) | |

### Shipped infrastructure

| # | Phase | What it unlocks |
|---|-------|----------------|
| 01 | Sandboxed Agents | Agents in Docker containers, controlled credentials, fork parity |
| 02 | Compose Stack | Full stack in Docker (lfd + postgres), test locally |
| 03 | Pre-shared Token Auth | lfd accepts remote connections |
| 04 | API Surface Gating | Security hardening for remote-facing API |
| — | Bundled Daemon Runtime | Concerto runs lfd from app bundle; ephemeral port/token per launch; refcounted per-repo sharing; connection mode switcher (Bundled/Remote) in settings UI |

### Folded into step 0

- **Phase 05** (Concerto Remote Connection): error mapping for daemon timeout/fail-fast, protocol parity validation
- **Phase 06** (Remote File Access): remote editor launch (Cursor/VSCode/Zed via SSH), remote terminal, "Copy SSH Command"

Smoke tests for SSE/WSS through Caddy TLS proxy happen during deployment (steps 1-2), not as a separate phase.

## Goals

- Keep protocol parity: Concerto talks to local and remote lfd via the same HTTP/WS surface *(validated: bundled and remote modes use identical handshake pipeline — TLS/auth/repo discovery/ws probe)*
- Keep orchestration ownership in loopflow while editors handle their native remote transport
- Ship remote connectivity incrementally: secure auth first, then UX and API breadth

## Risks

- **TLS proxy latency for SSE/WSS.** Real-time event streaming through Caddy TLS proxy adds latency that hasn't been validated at scale. Mitigate: smoke test SSE/WSS through Caddy during deployment steps 1-2, not as a separate phase.
- **Host-side git worktrees for prompt assembly.** Docker fork branches rely on host-side worktrees before container launch. Moving prompt assembly into containers would require container→host sync or a prompt build path that doesn't need host materialization. Acceptable for now.
- **Stale PR state without GitHub token.** Queue UX must degrade gracefully when no token is configured — treat as degraded mode, not hard failure.
- **Remote file access depends on editor SSH support.** Cursor/VSCode/Zed each have different Remote SSH implementations. If any breaks or changes behavior, the one-click workflow breaks for that editor. Mitigate: "Copy SSH Command" as universal fallback.
- **Agent timeout configuration surface.** `executor.agent_timeout` is daemon-config only. If users need per-wave or per-session timeouts, the config model needs rethinking. Defer until dogfooding reveals whether this is a real need.

## Update after Phase 01E (Docker fork parity)

Phases 01A–01E are fully shipped. What we now treat as non-negotiable for remote work:

- Fork semantics are the same in CLI, daemon, and Docker executor: run all branches, then synthesize.
- Docker forks use sibling worktrees in container volumes (`/workspace/repos/<repo>/worktrees/<wave>-fork-N`).
- Scheduler slot release and orphan-fork cleanup must be restart-safe (covers both native and Docker).
- Agent timeout is explicit operator config (`executor.agent_timeout`), not hidden watchdog behavior.
- Executor trait abstraction (`AgentExecutor`) handles workspace file ops — no Docker-specific downcasts in fork orchestration.

### Known limitation: host-side git worktrees for prompt assembly

Docker fork branches still rely on host-side git worktrees for prompt assembly (`build_step_prompt` needs local step/context files before container launch). This is acceptable — prompt assembly happens before container launch, and the worktrees are cleaned up after the fork completes. Moving to host placeholder dirs would require either pre-prompt container→host sync or a prompt build path that doesn't depend on host worktree materialization.

### Open questions for deployment

- Do we expose `executor.agent_timeout` in Concerto connection settings, or keep it daemon-config only in v1?
- Should remote capability warnings live in wave detail only, or also in wave edit/config surfaces?
- Connection settings now show Bundled/Remote mode. When studio auth ships, does "Remote" split into "Remote (static token)" and "Remote (studio auth)", or does studio auth replace static token for the Concerto UI path?

## Metrics

- Remote Concerto sessions can perform wave CRUD, event streaming, and log access over WAN
- SSE chat events and WebSocket run events both succeed through TLS proxy paths
- Remote workflows preserve local behavior expectations (same protocol semantics, auth-gated access)
- One-click wave-to-editor workflow works via Remote SSH
