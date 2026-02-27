# Remote

## Vision

Connect Concerto to a remote lfd. Same HTTP+WS protocol, different host. lfd runs on a remote Linux machine (containerized). Concerto connects from your Mac (or phone). Waves run remotely with full agent execution. You open Cursor via Remote SSH when you need to edit files.

## Strategy

Keep protocol parity: Concerto talks to local and remote lfd via the same HTTP/WS surface. Keep orchestration ownership in loopflow while editors handle their native remote transport. Ship remote connectivity incrementally: dogfood deployment first, eliminate fork drift, then studio auth and API breadth.

Infrastructure phases (01–04) shipped the foundation: sandboxed agents in Docker, Compose stack, pre-shared token auth, and API surface gating. Bundled daemon mode shipped as the default local connection, validating protocol parity end-to-end. EC2 dogfood lane (step 1) deployed and smoke-tested. File-based remote seeding from `~/.lf/concerto.yaml` + Keychain token lookup shipped.

### Multi-repo execution model

Remote delivery spans two repos:

- `loopflow` (this repo): lfd/Concerto protocol behavior, fork parity, remote smoke workflow.
- `studio` (private): auth server, EC2 host lifecycle, daemon discovery surfaces.

Keep ownership crisp: changes to HTTP/JWT/registration contracts must land with matching updates in both repos.

## Goals

- Keep protocol parity: Concerto talks to local and remote lfd via the same HTTP/WS surface
- Keep orchestration ownership in loopflow while editors handle their native remote transport
- Ship remote connectivity incrementally: dogfood deployment first, eliminate fork drift, then studio auth and API breadth

## Risks

- **TLS proxy latency for SSE/WSS.** Real-time event streaming through Caddy TLS proxy adds latency. EC2 smoke validates SSE/WS through Caddy but sustained load and reconnect under real latency are unvalidated.
- **Host-side git worktrees for prompt assembly.** Docker fork branches rely on host-side worktrees before container launch.
- **Remote file access depends on editor SSH support.** Each editor has different Remote SSH implementations. "Copy SSH Command" as universal fallback.
- **Remote bootstrap misconfig can look like generic connection failure.** Malformed `~/.lf/concerto.yaml` or missing Keychain token can land in a failing remote path silently.

## Metrics

- Concerto connects to remote lfd and shows wave list, live output, and chat identically to local
- Remote wave run completes end-to-end: start on Concerto, agent executes on remote, PR lands
- SSE/WS streaming through TLS proxy is reliable with reconnect recovery
- `lfd install` on a remote host provisions a working daemon reachable from Concerto
