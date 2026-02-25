# 01: EC2 Dogfood Lane

Run real remote waves from a laptop against EC2 before adding more surface area.

## What exists after this

One stable remote lane is live end-to-end:

- lfd runs on EC2 via Docker Compose + Caddy TLS
- Concerto connects from a laptop over WAN using static-token auth
- A repeatable smoke script exercises CRUD, run, logs, SSE/WS, and editor/terminal actions

This is a deployment-validation phase, not a feature phase.

## Scope

### In scope

- Deploy and document the EC2 stack in `../studio`
- Connect Concerto in Remote mode and run day-to-day workflows
- Capture operational failures and fix them in the owning repo
- Add or extend one runnable smoke script for EC2 verification

### Out of scope

- Studio JWT auth rollout
- New API endpoints for file browsing/typeahead
- Hosted multi-tenant packaging

## Test loop

Run from laptop against EC2:

1. Connect Concerto to remote host (TLS + static token)
2. Create/edit/delete a wave
3. Run at least one normal step and one forked flow
4. Verify live updates over SSE and WebSocket
5. Open worktree in remote editor and remote terminal
6. Verify logs and reconnect behavior after lfd restart

Record failures as either:

- protocol/runtime bugs (`loopflow.remote`)
- provisioning/auth/infrastructure bugs (`../studio`)

## Done when

- EC2 lane can be reprovisioned from docs without tribal knowledge
- Smoke run passes consistently from laptop
- SSE and WS survive Caddy TLS path under normal usage
- Blocking issues are fixed or explicitly tracked with owner + mitigation
- Team can dogfood this lane for real work without manual babysitting
