# Remote

Connect Concerto to a remote lfd. Same HTTP+WS protocol, different host.

## Vision

lfd runs on a remote Linux machine (containerized). Concerto connects from your Mac (or phone). Waves run remotely with full agent execution. You open Cursor via Remote SSH when you need to edit files. One-click per wave.

## Roadmap

Infrastructure phases (01-04) shipped the foundation. Bundled daemon mode shipped as the default local connection, validating protocol parity end-to-end. The next loop is operational: run remote day-to-day on real hosts, fix drift, then ship studio auth.

| Step | What | Status |
|------|------|--------|
| 0 | Error mapping + remote editor/terminal launch | Shipped |
| — | Bundled daemon runtime + connection mode switcher | Shipped |
| 1 | EC2 dogfood lane: deploy (Docker + Caddy + static token) and run remote smoke from laptop | Next |
| 2 | Mac Mini dogfood lane: deploy (native + launchd) and run the same smoke suite for parity | Next |
| 3 | Fork executor cleanup before auth rollout (shared constants, branch threading, executor hook parity) | Next |
| 4 | Studio auth (JWT validation, sign-in UX, JWKS hardening) | After 1-3 |
| 5 | API expansion (remote file browsing + metadata typeahead) | Later |
| 6 | Hosted SaaS packaging | Later |

## Multi-repo execution model

Remote delivery now spans two repos:

- `loopflow.remote` (this repo): lfd/Concerto protocol behavior, fork parity, remote smoke workflow, and operator-facing deployment docs.
- `../studio` (private mono repo): auth server, EC2 host lifecycle/provisioning, and studio-side daemon discovery surfaces.

Keep ownership crisp:

- Changes to HTTP/JWT/registration contracts must land with matching updates in both repos in the same iteration window.
- Deployment dogfood issues found from Concerto-on-laptop runs get logged/fixed in the owning repo, then re-verified from this wave.
- Studio auth is not "done" until both repos pass the same remote connection path end-to-end.

### Shipped infrastructure

| # | Phase | What it unlocks |
|---|-------|----------------|
| 01 | Sandboxed Agents | Agents in Docker containers, controlled credentials, fork parity |
| 02 | Compose Stack | Full stack in Docker (lfd + postgres), test locally |
| 03 | Pre-shared Token Auth | lfd accepts remote connections |
| 04 | API Surface Gating | Security hardening for remote-facing API |
| — | Bundled Daemon Runtime | Concerto runs one bundled lfd from the app bundle; ephemeral port/token per launch; shared sqlite db; connection mode switcher (Bundled/Remote) in settings UI |

Phase 05 (remote connection correctness) and Phase 06 (remote editor/terminal access) were folded into step 0 and are treated as shipped baseline behavior.

## Goals

- Keep protocol parity: Concerto talks to local and remote lfd via the same HTTP/WS surface *(validated: bundled and remote modes use identical handshake pipeline — TLS/auth/repo discovery/ws probe)*
- Keep orchestration ownership in loopflow while editors handle their native remote transport
- Ship remote connectivity incrementally: dogfood deployment first, eliminate fork drift, then studio auth and API breadth

## Risks

- **TLS proxy latency for SSE/WSS.** Real-time event streaming through Caddy TLS proxy adds latency that hasn't been validated at scale. Mitigate: smoke test SSE/WSS through Caddy during deployment steps 1-2, not as a separate phase.
- **Host-side git worktrees for prompt assembly.** Docker fork branches rely on host-side worktrees before container launch. Moving prompt assembly into containers would require container→host sync or a prompt build path that doesn't need host materialization. Acceptable for now.
- **Stale PR state without GitHub token.** Queue UX must degrade gracefully when no token is configured — treat as degraded mode, not hard failure.
- **Remote file access depends on editor SSH support.** Cursor/VSCode/Zed each have different Remote SSH implementations. If any breaks or changes behavior, the one-click workflow breaks for that editor. Mitigate: "Copy SSH Command" as universal fallback.
- **Agent timeout configuration surface.** `executor.agent_timeout` is daemon-config only. If users need per-wave or per-session timeouts, the config model needs rethinking. Defer until dogfooding reveals whether this is a real need.
- **Scope drift before dogfood.** New API/auth surface is easy to add before we have enough EC2/Mac Mini runtime data. Mitigate: run and fix deployment smoke first, then expand surface.
