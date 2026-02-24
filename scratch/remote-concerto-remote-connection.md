# 05/06: Remote Connection + File Access (Step 0)

## What shipped

Step 0 folds the concrete value from phases 05 and 06 into one commit:

### Error mapping (from phase 05)
- `WaveServiceError.daemonTimeout` — maps HTTP 502/504 from Caddy to actionable user messages
- `DisconnectReason.daemonTimeout` — "Agent timed out — check server logs" in connection status
- HTTP 502 → `.serverUnreachable` ("Daemon not responding")

### Remote editor/terminal launch (from phase 06)
- `RepoTarget.remote(path:host:)` — carries the connection host for SSH commands
- `TerminalLauncher.openInIDE` with `remoteHost` — Cursor/VSCode via `--remote ssh-remote+host`, Zed via `ssh://host/path`
- `TerminalLauncher.openTerminalRemote` — SSH session in user's terminal app
- `WaveDetailPanel` quickActionsBar works for both local and remote (no more "Local actions unavailable" notice)
- "Copy SSH Command" button replaces "Reveal in Finder" when remote
- Command palette and keyboard shortcuts work for remote connections

## What moved to deployment (steps 1-2)

- SSE/WSS smoke tests through Caddy TLS proxy — validated during actual EC2/Mac Mini deployment
- Caddy buffering regression testing — same, validated in production proxy config
- Reconnect stability testing — needs real WAN conditions

## Remaining roadmap

| Step | What |
|------|------|
| 1 | Deploy lfd to EC2 (Docker + Caddy + static token) |
| 2 | Deploy lfd to Mac Mini (native + launchd), connect Concerto |
| 3 | Studio auth (Phase 07) |
