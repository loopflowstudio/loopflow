# lfd Repos/Auth

Provider auth and repo onboarding surfaces for Concerto and `lfq`.

## Vision

Connecting GitHub, Claude, and Codex should be browser-first and identical across clients: click connect, finish auth, continue working.

## Roadmap

| Step | What | Status |
|------|------|--------|
| 1 | Provider auth broker in `lfd` + `lfq auth` commands | Shipped |
| 2 | Live CLI contract validation + auth-flow hardening (`gh`/`claude`) | In Progress |
| 3 | Concerto Connections panel wired to `/v0/auth` + auth events | Later |
| 4 | Repo onboarding (`POST /v0/repos`) and repo-first workflow | Later |

## Risks

- Provider CLI output changes can break URL/code parsing.
- Filesystem heuristics (`~/.claude`, `~/.codex`) may drift from real credential layouts.
- Missing host binaries (`gh`, `claude`, `codex`) blocks interactive auth flows.
