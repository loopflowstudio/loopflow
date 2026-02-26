# lfd Repos/Auth

Provider auth and repo onboarding surfaces for Concerto and `lfq`.

## Vision

Connecting GitHub, Claude, and Codex should be browser-first and identical across clients: click connect, finish auth, continue working.

## Roadmap

| Step | What | Status |
|------|------|--------|
| 1 | Provider auth broker in `lfd` + `lfq auth` commands | Shipped |
| 2 | Live CLI contract validation + auth-flow hardening (`gh`/`claude`) | Shipped |
| 3 | Concerto Connections panel wired to `/v0/auth` + auth events | Shipped |
| 4 | Repo onboarding (`POST /v0/repos`) and repo-first workflow | Shipped |

## Next

All four roadmap steps are shipped. Natural follow-ups:

- Concerto `PortfolioService` integration — use `GET/POST /v0/repos` as source of truth instead of UserDefaults-only.
- API contract alignment — confirm path-based vs name-based design for long-term target.
- Wave count scaling — `COUNT GROUP BY repo` query instead of fetching all waves.

## Risks

- Provider CLI output changes can break URL/code parsing. Mitigated by regex with fallbacks and `scripts/test_auth_live_contract.py`.
- Missing host binaries (`gh`, `claude`, `codex`) blocks interactive auth flows.
- Browser launch may fail silently during OAuth device flow; URL fallback with copy button degrades UX.
- `POST /v0/auth/{provider}` uses 30s/60s timeouts because the server spawns a CLI process — UI shows pending state for a while if the CLI is slow.
