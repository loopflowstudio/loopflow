---
linear_id: 55e6b79a-fe16-4e07-82e4-b1426d54a66e
---
# 01: Mac Mini Server

**Finish line:** lfd runs natively on Mac Mini via launchd, stays up across reboots, and behaves identically to local development.

## What to build

1. **launchd plist.** `lf ops install` generates and loads a LaunchDaemon. Auto-restart on crash. Log rotation via `stdout_path`/`stderr_path`.

2. **Remote behavior parity.** Every feature that works locally must work over the network. Specifically validate:
   - Wave creation, running, stopping via `lfq` pointed at remote lfd
   - WebSocket streaming (session output, wave logs)
   - Provider auth flows (OAuth callback must reach the Mac Mini)
   - File reads via API (area inspection, diff viewing)

3. **Monitoring.** Health endpoint (`GET /v0/health`) returns uptime, active waves, last error. Simple enough to curl from a cron job or Concerto's portfolio view.

## Done when

- lfd survives a reboot on Mac Mini
- `lfq` commands work from a laptop on the same network
- Concerto connects to remote lfd and shows wave state
- At least one wave runs a full build cycle remotely
