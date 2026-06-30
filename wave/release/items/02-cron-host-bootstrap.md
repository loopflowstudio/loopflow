---
priority: p1
status: open
---
# Cron host bootstrap

Bring up the first maintained self-hosted `lfd` cron host.

## Goal

A real host runs Loopflow crons from committed deploy files and Doppler secrets. Mac mini + Tailscale is the default first target unless another host is simpler at implementation time.

## Done when

- Host has Doppler configured outside git.
- `deploy/loopflow-server.sh up` or the host service unit starts `lfd` behind TLS/private networking.
- Root/conductor wave exists on the host with scheduled garden/release checks.
- Status, logs, and update commands are documented and runnable.
