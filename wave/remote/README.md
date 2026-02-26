# Remote

Connect Concerto to a remote lfd. Same HTTP+WS protocol, different host.

## Vision

lfd runs on a remote Linux machine (containerized). Concerto connects from your Mac (or phone). Waves run remotely with full agent execution. You open Cursor via Remote SSH when you need to edit files. One-click per wave.

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
