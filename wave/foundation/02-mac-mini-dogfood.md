# 02: Mac Mini Dogfood Lane

**Finish line:** Remote behavior parity proven on a native host (launchd), not only Docker-on-EC2.

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

Run `scripts/test_remote_smoke.py` against the Mac Mini target. The same script covers health, auth, wave CRUD, SSE, WS, run+logs, and reconnect. Call out differences:

- wave CRUD, run lifecycle, logs, SSE/WS
- fork execution + cleanup behavior
- remote editor/terminal launch behavior (manual — not covered by smoke script)
- first-launch Concerto bootstrap path (`~/.lf/concerto.yaml` + Keychain `<host>:<port>` token), including UserDefaults-wins seed behavior and loopback rejection
- config expectations (`executor.agent_timeout`, credentials, repo paths)

### Learnings from EC2

- Fresh hosts require explicit `--repo` flag — no `/v0/repos` fallback until a repo is registered.
- Session checks depend on a configured harness (`--session-harness`, default `claude`) on the remote host.
- TLS verification supports three modes: default, custom CA (`--ca-cert`), insecure (`--insecure`). Mac Mini may not need Caddy/TLS if running on a trusted network — document which mode applies.
- Slow hosts may need higher `--events-timeout` and `--logs-timeout` tuning.
- Concerto now supports file-based first-launch remote seeding; operators must ensure both `~/.lf/concerto.yaml` and matching Keychain token are present before smoke runs.

Any divergence from EC2 must be either:

- fixed, or
- documented as intentional with clear operator guidance

## Done when

- Mac Mini lane runs the same smoke suite as EC2 with comparable outcomes
- launchd lifecycle (boot/restart/crash recovery) is documented and tested
- Known EC2-only or Mac-only differences are explicit and minimal
- Remote bootstrap setup/diagnostics documented for operators
