# 02: Mac Mini Dogfood Lane

Prove remote behavior parity on a native host (launchd), not only Docker-on-EC2.

## What exists after this

A second production-like lane is stable:

- lfd runs natively on Mac Mini under `launchd`
- Concerto connects remotely from laptop
- The same smoke checks from EC2 pass with host-specific docs

## Scope

### In scope

- Native lfd service management on Mac Mini (`launchd`)
- Connection reliability and restart behavior
- Parity checks against EC2 lane behavior
- Scripted smoke validation from laptop

### Out of scope

- New auth model (studio JWT)
- New remote API breadth
- Hosted orchestration

## Parity checks

Use the same user flows as EC2 and call out differences:

- wave CRUD, run lifecycle, logs, SSE/WS
- fork execution + cleanup behavior
- remote editor/terminal launch behavior
- config expectations (`executor.agent_timeout`, credentials, repo paths)

Any divergence from EC2 must be either:

- fixed, or
- documented as intentional with clear operator guidance

## Done when

- Mac Mini lane runs the same smoke suite as EC2 with comparable outcomes
- launchd lifecycle (boot/restart/crash recovery) is documented and tested
- Known EC2-only or Mac-only differences are explicit and minimal
- Remote day-to-day use works on both lanes without special-case UI behavior
