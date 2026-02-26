# Remote

Connect Concerto to a remote lfd. Same HTTP+WS protocol, different host.

## Vision

lfd runs on a remote Linux machine (containerized). Concerto connects from your Mac (or phone). Waves run remotely with full agent execution. You open Cursor via Remote SSH when you need to edit files. One-click per wave.

## Roadmap

Infrastructure phases (01-04) shipped the foundation. Bundled daemon mode shipped as the default local connection, validating protocol parity end-to-end. The next loop is operational: run remote day-to-day on real hosts, fix drift, then ship studio auth.

| Step | What | Status |
|------|------|--------|
| 0 | Error mapping + remote editor/terminal launch | Shipped |
| — | Bundled daemon runtime (native + container modes) + connection mode switcher | Shipped |
| 1 | EC2 dogfood lane: deploy (Docker + Caddy + static token) and run remote smoke from laptop | Shipped |
| — | Concerto first-launch remote seeding from `~/.lf/concerto.yaml` + Keychain token lookup | Shipped |
| 2 | Mac Mini dogfood lane: deploy (native + launchd) and run the same smoke suite for parity | Next |
| 3 | Fork executor cleanup before auth rollout (shared constants, branch threading, executor hook parity) | Next |
| 4 | Studio auth (JWT validation, sign-in UX, JWKS hardening) | After 1-3 |
| 5 | API expansion (remote file browsing + metadata typeahead) | Later |
| 6 | Hosted SaaS packaging | Later |
| 7 | Bundled container hardening (auth flow orchestration, fallback policy, UI test stability) | In progress (auth flow done, 2 items remain) |

### Shipped infrastructure

| # | Phase | What it unlocks |
|---|-------|----------------|
| 01 | Sandboxed Agents | Agents in Docker containers, controlled credentials, fork parity |
| 02 | Compose Stack | Full stack in Docker (lfd + postgres), test locally |
| 03 | Pre-shared Token Auth | lfd accepts remote connections |
| 04 | API Surface Gating | Security hardening for remote-facing API |
| — | Bundled Daemon Runtime | Concerto runs one bundled lfd with ephemeral port/token per launch. Default path is a Dockerized `loopflow/lfd` runtime with `~/src` read-only, DB-backed credential injection, shared sqlite volume, and native fallback mode from settings. |
| — | File-Based Remote Seeding | On first launch, Concerto seeds remote host/port from `~/.lf/concerto.yaml` (`connection.host`/`connection.port`), enforces TLS + static-token auth, reads token from Keychain via `<host>:<port>`, and ignores loopback hosts |

Phase 05 (remote connection correctness) and Phase 06 (remote editor/terminal access) were folded into step 0 and are treated as shipped baseline behavior.

## Goals

- Keep protocol parity: Concerto talks to local and remote lfd via the same HTTP/WS surface
- Keep orchestration ownership in loopflow while editors handle their native remote transport
- Ship remote connectivity incrementally: dogfood deployment first, eliminate fork drift, then studio auth and API breadth

## Multi-repo execution model

Remote delivery spans two repos:

- `loopflow` (this repo): lfd/Concerto protocol behavior, fork parity, remote smoke workflow.
- `studio` (private): auth server, EC2 host lifecycle, daemon discovery surfaces.

Keep ownership crisp: changes to HTTP/JWT/registration contracts must land with matching updates in both repos.

## Risks

- **TLS proxy latency for SSE/WSS.** Real-time event streaming through Caddy TLS proxy adds latency. EC2 smoke validates SSE/WS through Caddy but sustained load and reconnect under real latency are unvalidated.
- **Host-side git worktrees for prompt assembly.** Docker fork branches rely on host-side worktrees before container launch.
- **Remote file access depends on editor SSH support.** Each editor has different Remote SSH implementations. "Copy SSH Command" as universal fallback.
- **Remote bootstrap misconfig can look like generic connection failure.** Malformed `~/.lf/concerto.yaml` or missing Keychain token can land in a failing remote path silently.
